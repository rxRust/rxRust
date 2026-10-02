//! Repeat operator implementation
//!
//! Resubscribes to the source when it completes.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Scheduler, Task, TaskState},
  subscription::{IntoBoxedSubscription, Subscription},
};

/// Repeat operator: Resubscribe to the source on completion
///
/// `count` is the total number of subscriptions (`Some(0)` completes
/// immediately, `Some(1)` is transparent); `None` repeats until
/// unsubscribed. Each resubscription happens on the scheduler's next tick,
/// so a synchronous source cannot recurse.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(not(target_arch = "wasm32"))]
/// # {
/// use rxrust::prelude::*;
///
/// # #[tokio::main(flavor = "local")]
/// # async fn main() {
/// Local::from_iter(vec![1, 2])
///   .repeat(3)
///   .subscribe(|v| println!("{}", v));
/// // Prints 1, 2, 1, 2, 1, 2 across three ticks
/// # }
/// # }
/// ```
#[derive(Clone)]
pub struct Repeat<S> {
  pub source: S,
  pub count: Option<usize>,
}

impl<S: ObservableType> ObservableType for Repeat<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that schedules the next subscription on completion
pub struct RepeatObserver<S, Ctx: Context> {
  source: S,
  observer: Ctx,
  remaining: Option<usize>,
  serial: Ctx::RcMut<Option<Ctx::BoxedSubscription>>,
  subscribe_fn: fn(Self),
}

impl<S: Clone, Ctx: Context + Clone> Clone for RepeatObserver<S, Ctx> {
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      observer: self.observer.clone(),
      remaining: self.remaining,
      serial: self.serial.clone(),
      subscribe_fn: self.subscribe_fn,
    }
  }
}

impl<S, Ctx> RepeatObserver<S, Ctx>
where
  Ctx: Context,
  S: CoreObservable<Ctx::With<Self>> + Clone,
  S::Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
{
  fn subscribe_impl(self) {
    let serial = self.serial.clone();
    // An empty slot means the downstream unsubscribed while the tick was
    // pending; do not resubscribe.
    let Some(previous) = serial.rc_deref_mut().take() else { return };
    previous.unsubscribe();
    let source = self.source.clone();
    let unsub = source.subscribe(Ctx::lift(self));
    *serial.rc_deref_mut() = Some(unsub.into_boxed());
  }
}

impl<S, Ctx, Item, Err> Observer<Item, Err> for RepeatObserver<S, Ctx>
where
  Self: Clone,
  Ctx: Context<Scheduler: Scheduler<Task<Option<Self>>>> + Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(mut self) {
    let again = match self.remaining {
      None => true,
      Some(n) if n > 1 => {
        self.remaining = Some(n - 1);
        true
      }
      Some(_) => false,
    };
    if !again || self.observer.is_closed() {
      self.observer.complete();
      return;
    }
    let scheduler = self.observer.scheduler().clone();
    scheduler.schedule(
      Task::new(Some(self), |this| {
        if let Some(observer) = this.take() {
          (observer.subscribe_fn)(observer);
        }
        TaskState::Finished
      }),
      None,
    );
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, Ctx> CoreObservable<Ctx> for Repeat<S>
where
  Ctx: Context,
  Ctx::Inner: for<'a> Observer<S::Item<'a>, S::Err>,
  S: CoreObservable<Ctx::With<RepeatObserver<S, Ctx>>> + Clone,
  S::Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
  Ctx::RcMut<Option<Ctx::BoxedSubscription>>: Subscription,
{
  type Unsub = Ctx::RcMut<Option<Ctx::BoxedSubscription>>;

  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let serial = Ctx::RcMut::from(None);
    if self.count == Some(0) {
      observer.into_inner().complete();
      return serial;
    }
    let repeat_observer = RepeatObserver {
      source: self.source,
      observer,
      remaining: self.count,
      serial: serial.clone(),
      subscribe_fn: RepeatObserver::subscribe_impl,
    };
    let unsub = repeat_observer
      .source
      .clone()
      .subscribe(Ctx::lift(repeat_observer));
    *serial.rc_deref_mut() = Some(unsub.into_boxed());
    serial
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  #[rxrust_macro::test]
  fn test_repeat_count() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let _sub = TestCtx::from_iter(vec![1, 2])
      .repeat(3)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    TestScheduler::flush();

    assert_eq!(*result.borrow(), vec![1, 2, 1, 2, 1, 2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_repeat_zero_completes_immediately() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let _sub = TestCtx::from_iter(vec![1])
      .repeat(0)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert!(result.borrow().is_empty());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_repeat_forever_unsubscribe_stops_resubscription() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let sub = TestCtx::from_iter(vec![7])
      .repeat_forever()
      .subscribe(move |v| result_c.borrow_mut().push(v));
    assert_eq!(*result.borrow(), vec![7]);

    // The next tick is already scheduled; unsubscribing must make it a no-op.
    sub.unsubscribe();
    TestScheduler::flush();

    assert_eq!(*result.borrow(), vec![7]);
    assert!(TestScheduler::is_empty());
  }

  #[rxrust_macro::test]
  fn test_repeat_error_stops() {
    TestScheduler::init();
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    let _sub = TestCtx::throw_err("boom".to_string())
      .map(|_| 0)
      .repeat(3)
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});
    TestScheduler::flush();

    assert_eq!(error.borrow().as_deref(), Some("boom"));
  }
}
