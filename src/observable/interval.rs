#![allow(unused_imports)]
use crate::observable::from_future::DEFAULT_RUNTIME;
use crate::observable::SharedOp;
use crate::prelude::*;
use futures::prelude::*;
use futures::{future::RemoteHandle, task::SpawnExt};
use futures_timer::Interval;
use std::time::{Duration, Instant};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
///
pub fn interval<O, Err>(
  dur: Duration,
) -> IntervalOp<impl IntervalFnOnce<O>, Err>
where
  O: Observer<usize, Err> + Send + Sync + 'static,
  Err: 'static,
{
  interval_observable_impl(move || Interval::new(dur))
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
///
pub fn interval_at<O, Err>(
  at: Instant,
  dur: Duration,
) -> IntervalOp<impl IntervalFnOnce<O>, Err>
where
  O: Observer<usize, Err> + Send + Sync + 'static,
  Err: 'static,
{
  interval_observable_impl(move || Interval::new_at(at, dur))
}

pub type IntervalOp<F, Err> = SharedOp<Observable<F, usize, Err>>;
pub trait IntervalFnOnce<O> = FnOnce(Subscriber<O, SharedSubscription>) + Clone;

fn interval_observable_impl<O, Err>(
  build_interval: impl FnOnce() -> Interval + Send + Sync + Clone + 'static,
) -> IntervalOp<impl IntervalFnOnce<O>, Err>
where
  O: Observer<usize, Err> + Send + Sync + 'static,
  Err: 'static,
{
  Observable::new(move |mut subscriber: Subscriber<O, SharedSubscription>| {
    let mut subscription = subscriber.subscription.clone();
    let mut number = 0;
    let f = build_interval().for_each(move |_| {
      subscriber.next(number);
      number += 1;
      future::ready(())
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for an interval failed");

    subscription.add(SpawnHandle::new(handle));
  })
  .to_shared()
}

pub struct SpawnHandle<T>(Option<RemoteHandle<T>>);

impl<T> SpawnHandle<T> {
  #[inline(always)]
  pub fn new(handle: RemoteHandle<T>) -> Self { SpawnHandle(Some(handle)) }
}

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
  interval(Duration::from_millis(20)).subscribe(move |_| {
    *seconds.lock().unwrap() += 1;
  });
  std::thread::sleep(Duration::from_millis(110));
  assert_eq!(*c_seconds.lock().unwrap(), 5);
}

#[test]
fn smoke_fork() {
  interval(Duration::from_millis(10))
    .fork()
    .to_shared()
    .fork()
    .subscribe(|_| {});
}
