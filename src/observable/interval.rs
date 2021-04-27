use crate::prelude::*;

use async_std::prelude::FutureExt;
use futures::prelude::*;
use std::{
  future::Future,
  time::{Duration, Instant},
};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
pub fn interval<S>(
  dur: Duration,
  scheduler: S,
) -> ObservableBase<IntervalEmitter<S>> {
  ObservableBase::new(IntervalEmitter {
    dur,
    at: Instant::now() + dur,
    scheduler,
  })
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
pub fn interval_at<S>(
  at: Instant,
  dur: Duration,
  scheduler: S,
) -> ObservableBase<IntervalEmitter<S>> {
  ObservableBase::new(IntervalEmitter { scheduler, dur, at })
}

#[derive(Clone)]
pub struct IntervalEmitter<S> {
  scheduler: S,
  dur: Duration,
  at: Instant,
}

impl<S> IntervalEmitter<S> {
  fn interval_future(
    &self,
    mut observer: impl Observer<Item = usize, Err = ()>,
  ) -> impl Future<Output = ()> {
    let mut number = 0;
    let now = Instant::now();
    let delay = if self.at > now {
      self.at - now
    } else {
      Duration::from_micros(0)
    };
    let dur = self.dur;

    future::ready(())
      .then(move |_| {
        observer.next(number);
        number += 1;
        async_std::stream::interval(dur).for_each(move |_| {
          observer.next(number);
          number += 1;
          future::ready(())
        })
      })
      .delay(delay)
  }
}

impl<S> Emitter for IntervalEmitter<S> {
  type Item = usize;
  type Err = ();
}

impl<S: SharedScheduler + 'static> SharedEmitter for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    let future = self.interval_future(subscriber.observer);
    let (future, handle) = futures::future::abortable(future);
    self.scheduler.spawn(future.map(|_| ()));
    subscriber.subscription.add(SpawnHandle::new(handle));
  }
}

impl<S: LocalScheduler + 'static> LocalEmitter<'static> for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    let future = self.interval_future(subscriber.observer);
    let (future, handle) = futures::future::abortable(future);
    self.scheduler.spawn(future.map(|_| ()));
    subscriber.subscription.add(SpawnHandle::new(handle));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};

  #[test]
  fn shared() {
    let seconds = Arc::new(Mutex::new(0));
    let c_seconds = seconds.clone();
    let pool = ThreadPool::new().unwrap();

    interval(Duration::from_millis(10), pool)
      .to_shared()
      .subscribe(move |_| {
        *seconds.lock().unwrap() += 1;
      });

    std::thread::sleep(Duration::from_millis(58));
    assert_eq!(*c_seconds.lock().unwrap(), 5);
  }

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let stamp = Instant::now();
    interval(Duration::from_millis(1), local.spawner())
      .take(5)
      .subscribe(|_| {});

    local.run();
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }
}
