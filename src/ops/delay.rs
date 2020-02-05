use crate::prelude::*;
use std::time::{Duration, Instant};

/// Delays the emission of items from the source Observable by a given timeout
/// or until a given `Instant`.
pub trait Delay {
  fn delay(self, dur: Duration) -> DelayOp<Self::Shared>
  where
    Self: Sized + IntoShared,
  {
    DelayOp {
      source: self.to_shared(),
      delay: dur,
    }
  }
  fn delay_at(self, at: Instant) -> DelayOp<Self::Shared>
  where
    Self: Sized + IntoShared,
  {
    DelayOp {
      source: self.to_shared(),
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

impl<O, U, Source> Observable<O, U> for DelayOp<Source>
where
  O: IntoShared,
  U: IntoShared + SubscriptionLike,
  U::Shared: SubscriptionLike,
  Source: Observable<O::Shared, U::Shared> + Send + Sync + 'static,
  Source::Unsub: Send + Sync + 'static,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let Self { delay, source } = self;
    let subscriber = subscriber.to_shared();

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
    .subscribe(move |v| {
      *value.lock().unwrap() = v;
    });
  assert_eq!(*c_value.lock().unwrap(), 0);
  std::thread::sleep(Duration::from_millis(51));
  assert_eq!(*c_value.lock().unwrap(), 1);
}
