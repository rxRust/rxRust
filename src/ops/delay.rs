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
impl<S> Delay for S {}

pub struct DelayOp<S> {
  source: S,
  delay: FDelay,
}

impl<S> IntoShared for DelayOp<S>
where
  S: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<Item, Err, Sub, S> RawSubscribable<Item, Err, Sub> for DelayOp<S>
where
  S:IntoShared,
  S::Shared: RawSubscribable<Item, Err, Sub::Shared>,
  Sub: IntoShared,
  <S::Shared as RawSubscribable<Item, Err, Sub::Shared>>::Unsub: IntoShared,
  <<S::Shared as RawSubscribable<Item, Err, Sub::Shared>>::Unsub as IntoShared>::Shared:
    SubscriptionLike,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(self, subscriber: Sub) -> Self::Unsub {
    let mut proxy = SharedSubscription::default();
    let mut c_proxy = proxy.clone();
    let Self { delay, source } = self;
    let source = source.to_shared();
    let subscriber = subscriber.to_shared();
    let f = delay.inspect(move |_| {
      proxy.add(source.raw_subscribe(subscriber).to_shared());
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for delay failed");
    c_proxy.add(SpawnHandle(Some(handle)));
    c_proxy
  }
}

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let value = Arc::new(Mutex::new(0));
  let c_value = value.clone();
  observable::of!(1)
    .delay(Duration::from_millis(50))
    .subscribe(move |v| {
      *value.lock().unwrap() = *v;
    });
  assert_eq!(*c_value.lock().unwrap(), 0);
  std::thread::sleep(Duration::from_millis(51));
  assert_eq!(*c_value.lock().unwrap(), 1);
}
