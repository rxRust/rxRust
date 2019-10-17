use crate::prelude::*;
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
      delay: dur,
    }
  }
  fn delay_at(self, at: Instant) -> DelayOp<Self>
  where
    Self: Sized,
  {
    DelayOp {
      source: self,
      delay: at.elapsed(),
    }
  }
}
impl<S> Delay for S {}

pub struct DelayOp<S> {
  source: S,
  delay: Duration,
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
  <<S::Shared as RawSubscribable<Item, Err, Sub::Shared>>::Unsub as IntoShared>
  ::Shared:
    SubscriptionLike,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(self, subscriber: Sub) -> Self::Unsub {
    let Self { delay, source } = self;
    let source = source.to_shared();
    let subscriber = subscriber.to_shared();

     Schedulers::ThreadPool.schedule(move |mut subscription, _| {
        subscription.add(source.raw_subscribe(subscriber).to_shared());
      },
      Some(delay),
      ())
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
