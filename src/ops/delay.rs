use crate::prelude::*;
use observable::observable_proxy_impl;
use std::time::Duration;

#[derive(Clone)]
pub struct DelayOp<S> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
}

observable_proxy_impl!(DelayOp, S);

impl<S> SharedObservable for DelayOp<S>
where
  S: SharedObservable + Send + Sync + 'static,
  S::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let Self { delay, source } = self;

    Schedulers::ThreadPool.schedule(
      move |mut subscription, _| {
        subscription.add(source.actual_subscribe(subscriber));
      },
      Some(delay),
      (),
    )
  }
}

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let value = Arc::new(Mutex::new(0));
  let c_value = value.clone();
  observable::of(1)
    .delay(Duration::from_millis(50))
    .to_shared()
    .subscribe(move |v| {
      *value.lock().unwrap() = v;
    });
  assert_eq!(*c_value.lock().unwrap(), 0);
  std::thread::sleep(Duration::from_millis(60));
  assert_eq!(*c_value.lock().unwrap(), 1);
}
