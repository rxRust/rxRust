use crate::observable::{from_future::DEFAULT_RUNTIME, interval::SpawnHandle};
use crate::prelude::*;
use futures::{future::FutureExt, task::SpawnExt};
use futures_timer::Delay as FDelay;
use std::time::{Duration, Instant};

/// Delays the emission of items from the source Observable by a given timeout
/// or until a given `Instant`.
pub trait Delay {
  fn delay(self, dur: Duration) -> DelayOp<Self>
  where
    Self: Sized,
  {
    DelayOp {
      source: self,
      delay: FDelay::new(dur),
    }
  }
  fn delay_at(self, at: Instant) -> DelayOp<Self>
  where
    Self: Sized,
  {
    DelayOp {
      source: self,
      delay: FDelay::new_at(at),
    }
  }
}
impl<S> Delay for S where S: RawSubscribable {}

pub struct DelayOp<S> {
  source: S,
  delay: FDelay,
}

impl<S> RawSubscribable for DelayOp<S>
where
  S: RawSubscribable + Send + 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let proxy = SubscriptionProxy::new();
    let c_proxy = proxy.clone();
    let Self { delay, source } = self;
    let f = delay.inspect(move |_| {
      proxy.proxy(source.raw_subscribe(subscribe));
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for delay failed");
    c_proxy.proxy(Box::new(SpawnHandle(Some(handle))));
    Box::new(c_proxy)
  }
}

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let value = Arc::new(Mutex::new(0));
  let c_value = value.clone();
  observable::of(1)
    .delay(Duration::from_millis(50))
    .subscribe(move |v| {
      *value.lock().unwrap() = *v;
    });
  assert_eq!(*c_value.lock().unwrap(), 0);
  std::thread::sleep(Duration::from_millis(51));
  assert_eq!(*c_value.lock().unwrap(), 1);
}
