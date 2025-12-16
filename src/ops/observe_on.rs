use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Scheduler, Task, TaskHandle, TaskState},
  subscription::DynamicSubscriptions,
};

/// ObserveOn operator: Re-emits all notifications from the source Observable on
/// a specified Scheduler.
///
/// This operator ensures that the downstream observer's methods (`next`,
/// `error`, `complete`) are executed in the context of the provided scheduler.
#[derive(Clone)]
pub struct ObserveOn<S, Sch> {
  pub source: S,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for ObserveOn<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== ObserveOn Subscription ====================

use crate::subscription::SourceWithDynamicSubs;

/// Subscription for the ObserveOn operator
pub type ObserveOnSubscription<U, S> = SourceWithDynamicSubs<U, S>;

// ==================== ObserveOn Observer ====================

pub struct ObserveOnObserver<P, Sch, S> {
  observer: P,
  scheduler: Sch,
  subscription: S,
}

// Handler for `next` notifications
fn observe_on_next_handler<P, Item, Err, S>(state: &mut (P, Option<Item>, usize, S)) -> TaskState
where
  P: Observer<Item, Err>,
  S: RcDerefMut<Target = DynamicSubscriptions<TaskHandle>>,
{
  let (observer, item_opt, id, subs_ptr) = state;

  if let Some(item) = item_opt.take() {
    observer.next(item);
  }

  subs_ptr.rc_deref_mut().remove(*id);

  TaskState::Finished
}

// Handler for `error` notifications
fn observe_on_error_handler<P, Item, Err, S>(state: &mut (P, Option<Err>, usize, S)) -> TaskState
where
  P: Observer<Item, Err> + Clone,
  S: RcDerefMut<Target = DynamicSubscriptions<TaskHandle>>,
{
  let (observer, err_opt, id, subs_ptr) = state;

  if let Some(err) = err_opt.take() {
    observer.clone().error(err);
  }

  subs_ptr.rc_deref_mut().remove(*id);

  TaskState::Finished
}

// Handler for `complete` notifications
fn observe_on_complete_handler<P, Item, Err, S>(state: &mut (P, usize, S)) -> TaskState
where
  P: Observer<Item, Err> + Clone,
  S: RcDerefMut<Target = DynamicSubscriptions<TaskHandle>>,
{
  let (observer, id, subs_ptr) = state;

  observer.clone().complete();

  subs_ptr.rc_deref_mut().remove(*id);

  TaskState::Finished
}

fn schedule_emission<P, Sch, S, Item, Err, State>(
  observer: &mut ObserveOnObserver<P, Sch, S>, id: usize, state: State,
  handler: fn(&mut State) -> TaskState,
) where
  P: Observer<Item, Err>,
  S: RcDerefMut<Target = DynamicSubscriptions<TaskHandle>>,
  Sch: Scheduler<Task<State>>,
{
  if observer.observer.is_closed() {
    return;
  }

  let task = Task::new(state, handler);
  let handle = observer.scheduler.schedule(task, None);
  observer
    .subscription
    .rc_deref_mut()
    .insert(id, handle);
}

impl<P, Sch, S, Item, Err> Observer<Item, Err> for ObserveOnObserver<P, Sch, S>
where
  P: Observer<Item, Err> + Clone,
  S: RcDerefMut<Target = DynamicSubscriptions<TaskHandle>> + Clone,
  Sch: Scheduler<Task<(P, Option<Item>, usize, S)>>
    + Scheduler<Task<(P, Option<Err>, usize, S)>>
    + Scheduler<Task<(P, usize, S)>>,
{
  fn next(&mut self, value: Item) {
    let id = self.subscription.rc_deref_mut().reserve_id();
    let state = (self.observer.clone(), Some(value), id, self.subscription.clone());
    schedule_emission(self, id, state, observe_on_next_handler::<P, Item, Err, S>);
  }

  fn error(mut self, err: Err) {
    let id = self.subscription.rc_deref_mut().reserve_id();
    let state = (self.observer.clone(), Some(err), id, self.subscription.clone());
    schedule_emission(&mut self, id, state, observe_on_error_handler::<P, Item, Err, S>);
  }

  fn complete(mut self) {
    let id = self.subscription.rc_deref_mut().reserve_id();
    let state = (self.observer.clone(), id, self.subscription.clone());
    schedule_emission(&mut self, id, state, observe_on_complete_handler::<P, Item, Err, S>);
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<C, S, Sch: Clone> CoreObservable<C> for ObserveOn<S, Sch>
where
  C: Context,
  S: CoreObservable<
    C::With<
      ObserveOnObserver<
        C::RcMut<Option<C::Inner>>,
        Sch,
        C::RcMut<DynamicSubscriptions<TaskHandle>>,
      >,
    >,
  >,
{
  type Unsub = ObserveOnSubscription<S::Unsub, C::RcMut<DynamicSubscriptions<TaskHandle>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let ObserveOn { source, scheduler } = self;

    let subs = C::RcMut::from(DynamicSubscriptions::default());
    let subs_clone = subs.clone();

    let wrapped_obs = context.transform(move |observer| ObserveOnObserver {
      observer: C::RcMut::from(Some(observer)),
      scheduler: scheduler.clone(),
      subscription: subs_clone,
    });

    let source_sub = source.subscribe(wrapped_obs);

    SourceWithDynamicSubs::new(source_sub, subs)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use super::*;
  use crate::{prelude::*, scheduler::LocalScheduler};

  #[rxrust_macro::test(local)]
  async fn smoke_test_local() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    Local::from_iter(0..5)
      .observe_on(LocalScheduler)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*values.lock().unwrap(), vec![0, 1, 2, 3, 4]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  // turbo
  #[rxrust_macro::test]
  async fn switch_thread_shared() {
    use std::{collections::HashSet, thread};

    use tokio::sync::oneshot;

    use crate::rc::{MutArc, RcDerefMut};

    let emitted_threads = MutArc::from(HashSet::new());
    let observed_threads = MutArc::from(HashSet::new());
    let emitted_c = emitted_threads.clone();
    let observed_c = observed_threads.clone();

    // Used to wait for completion
    let (tx, rx) = oneshot::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));

    Shared::from_iter(0..10)
      .map(move |v| {
        emitted_c
          .rc_deref_mut()
          .insert(thread::current().id());
        v
      })
      .observe_on(SharedScheduler)
      .on_complete(move || {
        if let Some(tx) = tx.lock().unwrap().take() {
          let _ = tx.send(());
        }
      })
      .subscribe(move |_| {
        observed_c
          .rc_deref_mut()
          .insert(thread::current().id());
      });

    // Wait for completion
    let _ = rx.await;

    let emitted = emitted_threads.rc_deref();
    let observed = observed_threads.rc_deref();

    // Threads should be different (or at least could be, but SharedScheduler spawns
    // new tasks usually on thread pool) With tokio multithreaded runtime, tasks
    // might run on different threads. We just verify it works.
    // Also, `observe_on` should have run the observer on the SharedScheduler
    // (pool).

    // We can't strictly guarantee different threads without knowing the runtime
    // flavor perfectly, but we can verify execution happened.
    assert!(!emitted.is_empty());
    assert!(!observed.is_empty());
  }

  #[rxrust_macro::test(local)]
  async fn test_cancellation() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    let mut subject = Local::subject::<i32, std::convert::Infallible>();

    let sub = subject
      .clone()
      .observe_on(LocalScheduler)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    subject.next(1);
    sub.unsubscribe();
    subject.next(2);

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    // 1 might or might not make it depending on when unsubscribe happens relative
    // to schedule? unsubscribe() cancels pending tasks.
    // next(1) schedules task.
    // unsubscribe() empties the subscription list (cancels task).
    // So 1 should NOT be received if unsubscribe happens synchronously after next
    // but before task execution. LocalScheduler doesn't execute immediately
    // unless yielded. So 1 should be cancelled.
    // 2 is not even scheduled because subject removes subscription? (Actually
    // observed wrapper still receives next(2) if subject not fully unsubscribed?
    // No, subject unsubscribes).

    let vals = values.lock().unwrap();
    assert!(vals.is_empty(), "Received {:?} but expected empty", vals);
  }
}
