use crate::{
  prelude::*,
  rc::{MutArc, MutRc},
};

#[derive(Debug, Clone)]
pub struct DelayOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

#[derive(Debug, Clone)]
pub struct DelayOpThreads<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

#[derive(Debug, Clone)]
pub struct DelaySubscriptionOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

pub struct DelayObserver<O, SD> {
  delay: Duration,
  scheduler: SD,
  observer: MutRc<Option<O>>,
  subscription: MultiSubscription<'static>,
}

pub struct DelayObserverThreads<O, SD> {
  delay: Duration,
  scheduler: SD,
  observer: MutArc<Option<O>>,
  subscription: MultiSubscriptionThreads,
}

impl<Item, Err, O, S, SD> Observable<Item, Err, O>
  for DelaySubscriptionOp<S, SD>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, O>,
  S::Unsub: 'static,
  SD: Scheduler<OnceTask<(S, O), SubscribeReturn<S::Unsub>>>,
{
  type Unsub = TaskHandle<SubscribeReturn<S::Unsub>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let task = OnceTask::new(subscribe_task, (self.source, observer));
    self.scheduler.schedule(task, Some(self.delay))
  }
}

macro_rules! impl_delay_op {
  ($op: ty, $rc: ident, $observer: ident, $multi_unsub: ty, $box_unsub: ident) => {
    impl<Item, Err, O, S, SD> Observable<Item, Err, O> for $op
    where
      O: Observer<Item, Err>,
      S: Observable<Item, Err, $observer<O, SD>>,
      SD: Scheduler<OnceTask<($rc<Option<O>>, Item), NormalReturn<()>>>,
      SD: Scheduler<OnceTask<$rc<Option<O>>, NormalReturn<()>>>,
    {
      type Unsub = ZipSubscription<S::Unsub, $multi_unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let Self { source, delay, scheduler } = self;
        let subscription: $multi_unsub = <_>::default();
        let observer = $rc::own(Some(observer));
        let observer = $observer {
          delay,
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
        let handler = self.scheduler.schedule(task, Some(self.delay));
        self.subscription.append($box_unsub::new(handler));
      }

      #[inline]
      fn error(self, err: Err) {
        self.observer.error(err)
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

        let handler = self.scheduler.schedule(task, Some(self.delay));
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

impl_delay_op!(DelayOp<S,SD>, MutRc, DelayObserver, MultiSubscription<'static>, BoxSubscription);
impl_delay_op!(DelayOpThreads<S,SD>, MutArc, DelayObserverThreads, MultiSubscriptionThreads, BoxSubscriptionThreads);

impl<Item, Err, S, SD> ObservableExt<Item, Err> for DelaySubscriptionOp<S, SD> where
  S: ObservableExt<Item, Err>
{
}

fn subscribe_task<S, O, Item, Err>(
  (source, observer): (S, O),
) -> SubscribeReturn<S::Unsub>
where
  S: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  SubscribeReturn::new(source.actual_subscribe(observer))
}

#[cfg(test)]
mod tests {
  use crate::rc::{MutRc, RcDeref, RcDerefMut};

  use super::*;
  use futures::executor::LocalPool;
  use std::{cell::RefCell, rc::Rc, time::Instant};

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared_smoke() {
    use futures::executor::ThreadPool;
    use std::sync::{Arc, Mutex};

    use crate::ops::complete_status::CompleteStatus;

    let value = Arc::new(Mutex::new(0));
    let c_value = value.clone();
    let pool = ThreadPool::new().unwrap();
    let stamp = Instant::now();
    let (o, status) = observable::of(1)
      .delay_threads(Duration::from_millis(10), pool)
      .complete_status();

    o.subscribe(move |v| {
      *value.lock().unwrap() = v;
    });
    CompleteStatus::wait_for_end(status);

    assert!(stamp.elapsed() >= Duration::from_millis(10));
    assert_eq!(*c_value.lock().unwrap(), 1);
  }

  #[test]
  fn local_smoke() {
    let value = Rc::new(RefCell::new(0));
    let c_value = value.clone();
    let mut pool = LocalPool::new();
    observable::of(1)
      .delay(Duration::from_millis(1), pool.spawner())
      .subscribe(move |v| {
        *c_value.borrow_mut() = v;
      });
    assert_eq!(*value.borrow(), 0);
    let stamp = Instant::now();
    pool.run();
    assert!(stamp.elapsed() >= Duration::from_millis(1));
    assert_eq!(*value.borrow(), 1);
  }

  #[test]
  fn delay_subscription_smoke() {
    let accept_stamp = MutRc::own(Instant::now());
    let c_accept_stamp = accept_stamp.clone();

    let mut pool = LocalPool::new();
    let mut subject = Subject::default();
    observable::of(1)
      .merge(subject.clone())
      .delay_subscription(Duration::from_millis(1), pool.spawner())
      .subscribe(move |_| *c_accept_stamp.rc_deref_mut() = Instant::now());
    pool.run();

    // the subscription delay.
    assert!(accept_stamp.rc_deref().elapsed() < Duration::from_millis(1));
    let emit_at = Instant::now();
    subject.next(0);
    pool.run();
    // emit not delay.
    assert!(accept_stamp.rc_deref().elapsed() < Duration::from_millis(1));
    assert!(
      accept_stamp.rc_deref().duration_since(emit_at)
        < Duration::from_millis(1)
    );
  }

  #[test]
  fn fix_delay_op_should_delay_value_emit() {
    let accept_stamp = MutRc::own(Instant::now());
    let c_accept_stamp = accept_stamp.clone();
    let mut pool = LocalPool::new();

    let mut subject = Subject::default();
    subject
      .clone()
      .delay(Duration::from_millis(1), pool.spawner())
      .subscribe(move |_| *c_accept_stamp.rc_deref_mut() = Instant::now());

    let emit_at = Instant::now();
    subject.next(());
    pool.run();

    assert!(
      accept_stamp.rc_deref().duration_since(emit_at)
        >= Duration::from_millis(1)
    );
    assert!(accept_stamp.rc_deref().elapsed() < Duration::from_millis(1));
  }
}
