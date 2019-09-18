use crate::observable::from_future::DEFAULT_RUNTIME;
use crate::prelude::*;
use futures::prelude::*;
use futures::{future::RemoteHandle, task::SpawnExt};
use futures_timer::Interval;
use std::time::{Duration, Instant};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
///
pub macro interval($dur: expr) {
  interval_observable!(Interval::new($dur))
}

/// Creates a observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
///
pub macro interval_at($at: expr, $dur: expr) {
  interval_observable!(Interval::new_at(at, dur))
}

macro interval_observable($interval: expr) {
  Observable::new(|mut subscriber: Subscriber<_, SharedSubscription>| {
    let mut subscription = subscriber.subscription.clone();
    let mut number = 0;
    let f = $interval.for_each(move |_| {
      subscriber.run(RxValue::Next(&()));
      future::ready(())
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for interval failed");
    let teardown: Box<SubscriptionLike + Send + Sync> =
      Box::new(SpawnHandle(Some(handle)));

    subscription.add(teardown);
  })
  .to_shared()
}

pub struct SpawnHandle<T>(pub(crate) Option<RemoteHandle<T>>);
impl<T> SubscriptionLike for SpawnHandle<T> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.0.take(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { self.0.is_none() }
}

impl<T> Drop for SpawnHandle<T> {
  fn drop(&mut self) {
    if self.0.is_some() {
      self.0.take().unwrap().forget()
    }
  }
}

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let seconds = Arc::new(Mutex::new(0));
  let c_seconds = seconds.clone();
  interval!(Duration::from_millis(10)).subscribe(move |_| {
    *seconds.lock().unwrap() += 1;
  });
  std::thread::sleep(Duration::from_millis(55));
  assert_eq!(*c_seconds.lock().unwrap(), 5);
}

#[test]
fn smoke_fork() {
  interval!(Duration::from_millis(10))
    .fork()
    .to_shared()
    .fork()
    .subscribe(|_| {});
}
