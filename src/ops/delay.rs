use crate::prelude::*;
use observable::observable_proxy_impl;
use std::time::Duration;

#[derive(Clone)]
pub struct DelayOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(DelayOp, S, SD);

macro impl_observable($op: ident, $subscriber: ident) {{
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
}}
impl<S, SD> SharedObservable for DelayOp<S, SD>
where
  S: SharedObservable + Send + Sync + 'static,
  S::Unsub: Send + Sync,
  SD: SharedScheduler,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
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
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'static>(
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
    observable::of(1)
      .delay(Duration::from_millis(50), pool)
      .to_shared()
      .subscribe(move |v| {
        *value.lock().unwrap() = v;
      });
    assert_eq!(*c_value.lock().unwrap(), 0);
    std::thread::sleep(Duration::from_millis(60));
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
    pool.run();
    assert_eq!(*value.borrow(), 1);
  }
}
