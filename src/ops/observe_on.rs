#[cfg(not(feature = "wasm-scheduler"))]
use crate::scheduler::SharedScheduler;
use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
#[derive(Clone)]
pub struct ObserveOnOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

impl<S: Observable, SD> Observable for ObserveOnOp<S, SD> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, SD> ObserveOnOp<S, SD>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let observer = ObserveOnObserver {
      observer: $ctx::Rc::own($observer),
      scheduler: $self.scheduler,
      subscription: $ctx::RcMultiSubscription::default(),
    };

    $self.source.actual_subscribe(observer)
  }
  where
    @ctx::local_only('o: 'static,)
    @ctx::shared_only(
      S::Item: Send + Sync +'static,
      S::Err: Send + Sync + 'static,
    )
    SD: @ctx::Scheduler @ctx::shared_only(+ Send + Sync) + 'static,
    S: @ctx::Observable

}

struct ObserveOnObserver<O, SD, S> {
  observer: O,
  scheduler: SD,
  subscription: S,
}

macro_rules! impl_observer {
  () => {
    type Item = O::Item;
    type Err = O::Err;

    fn next(&mut self, value: Self::Item) {
      self.observer_schedule(move |mut observer, v| observer.next(v), value)
    }
    fn error(&mut self, err: Self::Err) {
      self.observer_schedule(|mut observer, v| observer.error(v), err)
    }
    fn complete(&mut self) {
      self.observer_schedule(|mut observer, _| observer.complete(), ())
    }
  };
}

#[cfg(not(feature = "wasm-scheduler"))]
impl<O, SD> Observer for ObserveOnObserver<MutArc<O>, SD, SharedSubscription>
where
  O: Observer + Send + 'static,
  O::Item: Send + 'static,
  O::Err: Send + 'static,
  SD: SharedScheduler,
{
  impl_observer!();
}

impl<O, SD> Observer for ObserveOnObserver<MutRc<O>, SD, LocalSubscription>
where
  O: Observer + 'static,
  SD: LocalScheduler,
{
  impl_observer!();
}

macro_rules! impl_scheduler {
  (
    $scheduler_bound:ident,
    $rc: ident,
    $t_subscription: ty,
    $($send: ident)?
  ) => {
    impl<O, SD> ObserveOnObserver<$rc<O>, SD, $t_subscription>
    where
      SD: $scheduler_bound,
    {
      fn observer_schedule<S, Task>(&mut self, task: Task, state: S)
      where
        S:$($send + )? 'static,
        O:$($send + )? 'static,
        Task: FnOnce($rc<O>, S) +$($send + )? 'static,
      {
        let subscription = self.scheduler.schedule(
          |(observer, state)| task(observer, state),
          None,
          (self.observer.clone(), state),
        );

        self.subscription.add(subscription);
      }
    }
  };
}

#[cfg(not(feature = "wasm-scheduler"))]
impl_scheduler!(SharedScheduler, MutArc, SharedSubscription, Send);
impl_scheduler!(LocalScheduler, MutRc, LocalSubscription,);

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::collections::HashSet;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let v = Rc::new(RefCell::new(0));
    let v_c = v.clone();
    let mut local = LocalPool::new();
    observable::of(1)
      .observe_on(local.spawner())
      .subscribe(move |i| *v_c.borrow_mut() = i);
    local.run();

    assert_eq!(*v.borrow(), 1);
  }

  #[test]
  fn switch_thread() {
    let id = thread::spawn(move || {}).thread().id();
    let changed_thread = Arc::new(AtomicBool::default());
    let c_changed_thread = changed_thread.clone();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(HashSet::new()));
    let thread_clone = observe_thread.clone();

    let pool = ThreadPool::builder().pool_size(100).create().unwrap();

    observable::create(|s| {
      while !changed_thread.load(Ordering::Relaxed) {
        s.next(());
        *emit_thread.lock().unwrap() = thread::current().id();
      }
      s.complete();
    })
    .observe_on(pool)
    .into_shared()
    .subscribe_blocking(move |_v| {
      let mut thread = observe_thread.lock().unwrap();
      thread.insert(thread::current().id());

      c_changed_thread.store(thread.len() > 1, Ordering::Relaxed);
    });

    let current_id = thread::current().id();
    assert_eq!(*emit_thread.lock().unwrap(), current_id);
    let thread = thread_clone.lock().unwrap();
    assert!(thread.len() > 1);
  }

  #[test]
  fn pool_unsubscribe() {
    let scheduler = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .into_shared()
      .observe_on(scheduler.clone())
      .delay(Duration::from_millis(10), scheduler)
      .into_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_observe_on);

  fn bench_observe_on(b: &mut bencher::Bencher) { b.iter(smoke); }
}
