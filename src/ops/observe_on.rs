use crate::{
  prelude::*,
  rc::{MutArc, MutRc},
  scheduler::{NormalReturn, OnceTask, Scheduler},
};
#[derive(Clone)]
pub struct ObserveOnOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

pub struct ObserveOnObserver<O, SD> {
  observer: MutRc<Option<O>>,
  scheduler: SD,
  subscription: MultiSubscription<'static>,
}

#[derive(Clone)]
pub struct ObserveOnOpThreads<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

pub struct ObserveOnObserverThreads<O, SD> {
  observer: MutArc<Option<O>>,
  scheduler: SD,
  subscription: MultiSubscriptionThreads,
}

macro_rules! impl_observer_on_op {
  ($op: ty, $rc: ident, $observer: ident, $multi_unsub: ty, $box_unsub: ident) => {
    impl<Item, Err, O, S, SD> Observable<Item, Err, O> for $op
    where
      O: Observer<Item, Err>,
      S: Observable<Item, Err, $observer<O, SD>>,
      SD: Scheduler<OnceTask<($rc<Option<O>>, Item), NormalReturn<()>>>,
      SD: Scheduler<OnceTask<($rc<Option<O>>, Err), NormalReturn<()>>>,
      SD: Scheduler<OnceTask<$rc<Option<O>>, NormalReturn<()>>>,
    {
      type Unsub = ZipSubscription<S::Unsub, $multi_unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let Self { source, scheduler } = self;
        let subscription: $multi_unsub = <_>::default();
        let observer = $rc::own(Some(observer));
        let observer = $observer {
          scheduler,
          observer,
          subscription: subscription.clone(),
        };
        let unsub = source.actual_subscribe(observer);
        ZipSubscription::new(unsub, subscription)
      }
    }

    impl<Item, Err, O, SD> Observer<Item, Err> for $observer<O, SD>
    where
      O: Observer<Item, Err>,
      SD: Scheduler<OnceTask<($rc<Option<O>>, Item), NormalReturn<()>>>,
      SD: Scheduler<OnceTask<($rc<Option<O>>, Err), NormalReturn<()>>>,
      SD: Scheduler<OnceTask<$rc<Option<O>>, NormalReturn<()>>>,
    {
      fn next(&mut self, value: Item) {
        fn delay_emit_value<Item, Err>(
          (mut observer, value): (impl Observer<Item, Err>, Item),
        ) -> NormalReturn<()> {
          observer.next(value);
          NormalReturn::new(())
        }

        let observer = self.observer.clone();
        let task = OnceTask::new(delay_emit_value, (observer, value));
        self.subscription.retain();
        let handler = self.scheduler.schedule(task, None);
        self.subscription.append($box_unsub::new(handler));
      }

      #[inline]
      fn error(mut self, err: Err) {
        fn delay_emit_err<Item, Err>(
          (observer, err): (impl Observer<Item, Err>, Err),
        ) -> NormalReturn<()> {
          observer.error(err);
          NormalReturn::new(())
        }

        let observer = self.observer.clone();
        let task = OnceTask::new(delay_emit_err, (observer, err));
        self.subscription.retain();
        let handler = self.scheduler.schedule(task, None);
        self.subscription.append($box_unsub::new(handler));
      }

      #[inline]
      fn complete(mut self) {
        fn delay_complete<Item, Err>(
          observer: impl Observer<Item, Err>,
        ) -> NormalReturn<()> {
          observer.complete();
          NormalReturn::new(())
        }

        let observer = self.observer.clone();
        let task = OnceTask::new(delay_complete, observer);
        self.subscription.retain();

        let handler = self.scheduler.schedule(task, None);
        self.subscription.append($box_unsub::new(handler));
      }

      #[inline]
      fn is_finished(&self) -> bool {
        self.observer.is_finished()
      }
    }

    impl<Item, Err, S, SD> ObservableExt<Item, Err> for $op where
      S: ObservableExt<Item, Err>
    {
    }
  };
}

impl_observer_on_op!(ObserveOnOp<S,SD>, MutRc, ObserveOnObserver, MultiSubscription<'static>, BoxSubscription);

impl_observer_on_op!(ObserveOnOpThreads<S,SD>, MutArc, ObserveOnObserverThreads,
   MultiSubscriptionThreads, BoxSubscriptionThreads);

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::LocalPool;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  #[cfg(not(target_arch = "wasm32"))]
  use std::collections::HashSet;
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::atomic::{AtomicBool, Ordering};
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::{Arc, Mutex};
  #[cfg(not(target_arch = "wasm32"))]
  use std::thread;
  #[cfg(not(target_arch = "wasm32"))]
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

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn switch_thread() {
    use crate::ops::complete_status::CompleteStatus;

    let id = thread::spawn(move || {}).thread().id();
    let changed_thread = Arc::new(AtomicBool::default());
    let c_changed_thread = changed_thread.clone();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(HashSet::new()));
    let thread_clone = observe_thread.clone();

    let pool = ThreadPool::builder().pool_size(100).create().unwrap();

    let (o, status) = observable::create(|mut p: SubscriberThreads<_>| {
      while !changed_thread.load(Ordering::Relaxed) {
        p.next(());
        *emit_thread.lock().unwrap() = thread::current().id();
      }
      p.complete();
    })
    .observe_on_threads(pool)
    .complete_status();
    let _ = o.subscribe(move |_v| {
      let mut thread = observe_thread.lock().unwrap();
      thread.insert(thread::current().id());

      c_changed_thread.store(thread.len() > 1, Ordering::Relaxed);
    });

    CompleteStatus::wait_for_end(status);

    let current_id = thread::current().id();
    assert_eq!(*emit_thread.lock().unwrap(), current_id);
    let thread = thread_clone.lock().unwrap();
    assert!(thread.len() > 1);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn pool_unsubscribe() {
    let scheduler = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .delay_threads(Duration::from_millis(10), scheduler.clone())
      .observe_on_threads(scheduler)
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_observe_on);

  fn bench_observe_on(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }
}
