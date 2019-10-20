#![allow(unused_imports)]
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

#[allow(unused_macros)]
macro interval_observable($interval: expr) {
  Observable::new(|mut subscriber: Subscriber<_, SharedSubscription>| {
    let mut subscription = subscriber.subscription.clone();
    let mut number = 0;
    let f = $interval.for_each(move |_| {
      subscriber.next(&number);
      number += 1;
      future::ready(())
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for interval failed");

    subscription.add(SpawnHandle(Some(handle)));
  })
  .to_shared()
}

pub struct SpawnHandle<T>(pub(crate) Option<RemoteHandle<T>>);
impl<T> SubscriptionLike for SpawnHandle<T> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.0.take(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { self.0.is_none() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { ((&self.0) as *const _) as *const () }
}

impl<T> IntoShared for SpawnHandle<T>
where
  T: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
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
