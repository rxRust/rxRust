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
    impl<Item, Err, O, S, SD> ObservableImpl<Item, Err, O> for $op
    where
      O: Observer<Item, Err>,
      S: ObservableImpl<Item, Err, $observer<O, SD>>,
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

    impl<Item, Err, S, SD> Observable<Item, Err> for $op where
      S: Observable<Item, Err>
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
  use std::collections::HashSet;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;
  use std::{cell::RefCell, rc::Rc};

  #[tokio::test]
  async fn smoke() {
    let v = Rc::new(RefCell::new(0));
    let v_c = v.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::of(1)
        .observe_on(LocalScheduler)
        .subscribe(move |i| *v_c.borrow_mut() = i);
      local.await;
    }

    assert_eq!(*v.borrow(), 1);
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn switch_thread() {
    let emit_thread = Arc::new(Mutex::new(HashSet::new()));
    let observe_thread = Arc::new(Mutex::new(HashSet::new()));
    let emit_thread_c = emit_thread.clone();
    let observe_thread_c = observe_thread.clone();

    let (o, status) = observable::from_iter(0..50)
      .map(move |v| {
        emit_thread_c.lock().unwrap().insert(thread::current().id());
        v
      })
      .observe_on_threads(SharedScheduler)
      .complete_status();
    o.subscribe(move |_v| {
      observe_thread_c
        .lock()
        .unwrap()
        .insert(thread::current().id());
    });

    status.wait_completed().await;

    let emit = emit_thread.lock().unwrap();
    let observe = observe_thread.lock().unwrap();
    assert!(observe.iter().any(|id| !emit.contains(id)));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn pool_unsubscribe() {
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .delay_threads(Duration::from_millis(10), SharedScheduler)
      .observe_on_threads(SharedScheduler)
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    tokio::time::sleep(Duration::from_millis(20)).await;
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
