use crate::observable::from_future::DEFAULT_RUNTIME;
use crate::prelude::*;
use futures::prelude::*;
use futures::{future::RemoteHandle, task::SpawnExt};
use futures_timer::Interval;
use std::time::{Duration, Instant};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
///
#[inline(always)]
pub fn interval(dur: Duration) -> Interval { Interval::new(dur) }

/// Creates a observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
#[inline(always)]
pub fn interval_at(at: Instant, dur: Duration) -> Interval {
  Interval::new_at(at, dur)
}

impl RawSubscribable for Interval {
  type Item = ();
  type Err = ();
  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
    + Send
    + Sync
    + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let f = self.for_each(move |_| {
      subscribe.call((RxValue::Next(&()),));
      future::ready(())
    });
    let handle = DEFAULT_RUNTIME
      .lock()
      .unwrap()
      .spawn_with_handle(f)
      .expect("spawn future for interval failed");
    Box::new(SpawnHandle(Some(handle)))
  }
}

pub struct SpawnHandle<T>(pub(crate) Option<RemoteHandle<T>>);
impl<T> Subscription for SpawnHandle<T> {
  fn unsubscribe(&mut self) { self.0.take(); }
}

impl<T> Drop for SpawnHandle<T> {
  fn drop(&mut self) {
    if self.0.is_some() {
      self.0.take().unwrap().forget()
    }
  }
}

impl Multicast for Interval {
  type Output = Subject<(), ()>;
  #[inline(always)]
  fn multicast(self) -> Self::Output { self.into_subject() }
}

#[test]
fn smoke() {
  use std::sync::{Arc, Mutex};
  let seconds = Arc::new(Mutex::new(0));
  let c_seconds = seconds.clone();
  interval(Duration::from_millis(10)).subscribe(move |_| {
    *seconds.lock().unwrap() += 1;
  });
  std::thread::sleep(Duration::from_millis(55));
  assert_eq!(*c_seconds.lock().unwrap(), 5);
}
