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
pub fn interval(dur: Duration) -> ObservableBase<IntervalEmitter> {
  ObservableBase::new(IntervalEmitter {
    dur,
    at: Instant::now(),
  })
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
///
pub fn interval_at(
  at: Instant,
  dur: Duration,
) -> ObservableBase<IntervalEmitter> {
  ObservableBase::new(IntervalEmitter { dur, at })
}

#[derive(Clone)]
pub struct IntervalEmitter {
  dur: Duration,
  at: Instant,
}

impl SharedEmitter for IntervalEmitter {
  type Item = usize;
  type Err = ();
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let Subscriber {
      mut observer,
      mut subscription,
    } = subscriber;
    let mut number = 0;
    let f = Interval::new_at(self.at, self.dur).for_each(move |_| {
      observer.next(number);
      number += 1;
      future::ready(())
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for an interval failed");

    subscription.add(SpawnHandle::new(handle));
  }
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

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let seconds = Arc::new(Mutex::new(0));
  let c_seconds = seconds.clone();
  let _guard =
    interval(Duration::from_millis(20))
      .shared()
      .subscribe(move |_| {
        *seconds.lock().unwrap() += 1;
      });
  std::thread::sleep(Duration::from_millis(110));
  assert_eq!(*c_seconds.lock().unwrap(), 5);
}

#[test]
fn smoke_fork() {
  interval(Duration::from_millis(10))
    .clone()
    .clone()
    .shared()
    .subscribe(|_| {});
}
