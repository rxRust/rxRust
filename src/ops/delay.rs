use crate::prelude::*;
use std::time::Duration;

#[derive(Clone)]
pub struct DelayOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(DelayOp, S, SD);

macro_rules! impl_observable {
  ($op: ident, $subscriber: ident) => {{
    let delay = $op.delay;
    let source = $op.source;
    let scheduler = $op.scheduler;
    let subscription = $subscriber.subscription.clone();
    let c_subscription = subscription.clone();
    let handle = scheduler.schedule(
      move |_| {
        c_subscription.add(source.actual_subscribe($subscriber));
      },
      Some(delay),
      (),
    );
    subscription.add(handle);
    subscription
  }};
}
impl<S, SD> SharedObservable for DelayOp<S, SD>
where
  S: SharedObservable + Send + Sync + 'static,
  S::Unsub: Send + Sync,
  SD: SharedScheduler,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    impl_observable!(self, subscriber)
  }
}

impl<S, SD, Unsub> LocalObservable<'static> for DelayOp<S, SD>
where
  S: LocalObservable<'static, Unsub = Unsub> + 'static,
  Unsub: SubscriptionLike + 'static,
  SD: LocalScheduler,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  >(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    impl_observable!(self, subscriber)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::time::Instant;
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  #[test]
  fn shared_smoke() {
    let value = Arc::new(Mutex::new(0));
    let c_value = value.clone();
    let pool = ThreadPool::new().unwrap();
    let stamp = Instant::now();
    observable::of(1)
      .delay(Duration::from_millis(50), pool)
      .into_shared()
      .subscribe_blocking(move |v| {
        *value.lock().unwrap() = v;
      });
    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*c_value.lock().unwrap(), 1);
  }

  #[test]
  fn local_smoke() {
    let value = Rc::new(RefCell::new(0));
    let c_value = value.clone();
    let mut pool = LocalPool::new();
    observable::of(1)
      .delay(Duration::from_millis(50), pool.spawner())
      .subscribe(move |v| {
        *c_value.borrow_mut() = v;
      });
    assert_eq!(*value.borrow(), 0);
    let stamp = Instant::now();
    pool.run();
    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*value.borrow(), 1);
  }
}
