use crate::observable::from_future::DEFAULT_RUNTIME;
use crate::prelude::*;
use futures::prelude::*;
use futures::{future::RemoteHandle, task::SpawnExt};
use futures_timer::Interval;
use std::time::Duration;

/// Creates an Observable that emit sequential numbers every specified
/// interval of time.
///
#[inline(always)]
pub fn interval(dur: Duration) -> Interval { Interval::new(dur) }

impl RawSubscribable for Interval {
  type Item = ();
  type Err = ();
  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
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

pub struct SpawnHandle<T>(Option<RemoteHandle<T>>);
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
  interval(Duration::from_millis(5)).subscribe(move |_| {
    *seconds.lock().unwrap() += 1;
  });
  std::thread::sleep(Duration::from_millis(50));
  assert_eq!(*c_seconds.lock().unwrap(), 10);
}
