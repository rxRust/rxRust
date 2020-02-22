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

impl<S> SharedObservable for DelayOp<S>
where
  S: SharedObservable + Send + Sync + 'static,
  S::Unsub: Send + Sync,
{
  type Item = S::Item;
  type Err = S::Err;
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
