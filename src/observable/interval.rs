use crate::prelude::*;
use futures::{
  prelude::*,
  task::{LocalSpawnExt, SpawnExt},
};
use futures_timer::Delay;
use std::time::{Duration, Instant};

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
  ObservableBase::new(IntervalEmitter { dur, at, scheduler })
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
    mut observer: impl Observer<usize, ()>,
  ) -> impl Future<Output = ()> {
    let mut number = 0;
    let now = Instant::now();
    let delay = if self.at > now {
      self.at - now
    } else {
      Duration::from_micros(0)
    };
    let dur = self.dur;

    Delay::new(delay).then(move |_| {
      observer.next(number);
      number += 1;
      async_std::stream::interval(dur).for_each(move |_| {
        observer.next(number);
        number += 1;
        future::ready(())
      })
    })
  }
}

impl<S> Emitter for IntervalEmitter<S> {
  type Item = usize;
  type Err = ();
}

impl<S: SpawnExt + 'static> SharedEmitter for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let Subscriber {
      observer,
      mut subscription,
    } = subscriber;
    let f = self.interval_future(observer);
    let handle = self
      .scheduler
      .spawn_with_handle(f)
      .expect("spawn future for an interval failed");

    subscription.add(SpawnHandle::new(handle));
  }
}

impl<S: LocalSpawnExt + 'static> LocalEmitter<'static> for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + 'static,
  {
    let Subscriber {
      observer,
      mut subscription,
    } = subscriber;
    let f = self.interval_future(observer);
    let handle = self
      .scheduler
      .spawn_local_with_handle(f)
      .expect("spawn future for an interval failed");

    subscription.add(SpawnHandle::new(handle));
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

    std::thread::sleep(Duration::from_millis(55));
    assert_eq!(*c_seconds.lock().unwrap(), 5);
  }

  #[test]
  fn local() {
    unimplemented!();
  }
}
