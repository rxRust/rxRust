use crate::prelude::*;

use std::time::{Duration, Instant};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
pub fn interval<S>(
  dur: Duration,
  scheduler: S,
) -> ObservableBase<IntervalEmitter<S>> {
  ObservableBase::new(IntervalEmitter {
    dur,
    at: None,
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
  ObservableBase::new(IntervalEmitter {
    scheduler,
    dur,
    at: Some(at),
  })
}

#[derive(Clone)]
pub struct IntervalEmitter<S> {
  scheduler: S,
  dur: Duration,
  at: Option<Instant>,
}

impl<S> Emitter for IntervalEmitter<S> {
  type Item = usize;
  type Err = ();
}

impl<S: SharedScheduler + 'static> SharedEmitter for IntervalEmitter<S> {
  type Unsub = SpawnHandle;
  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    self.scheduler.schedule_repeating(
      move |i| observer.next(i),
      self.dur,
      self.at,
    )
  }
}

impl<S: LocalScheduler + 'static> LocalEmitter<'static> for IntervalEmitter<S> {
  type Unsub = SpawnHandle;
  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = usize, Err = Self::Err> + 'static,
  {
    self.scheduler.schedule_repeating(
      move |i| observer.next(i),
      self.dur,
      self.at,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_scheduler::ManualScheduler;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};

  #[test]
  fn shared() {
    let millis = Arc::new(Mutex::new(0));
    let c_millis = millis.clone();
    let stamp = Instant::now();
    let pool = ThreadPool::new().unwrap();

    interval(Duration::from_millis(1), pool)
      .take(5) // Will block forever if we don't limit emissions
      .into_shared()
      .subscribe_blocking(move |_| {
        *millis.lock().unwrap() += 1;
      });

    assert_eq!(*c_millis.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let stamp = Instant::now();
    let ticks = Arc::new(Mutex::new(0));
    let ticks_c = Arc::clone(&ticks);
    interval(Duration::from_millis(1), local.spawner())
      .take(5)
      .subscribe(move |_| (*ticks_c.lock().unwrap()) += 1);
    local.run();
    assert_eq!(*ticks.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }

  #[test]
  fn local_manual() {
    let scheduler = ManualScheduler::now();
    let ticks = Arc::new(Mutex::new(0));
    let ticks_c = Arc::clone(&ticks);
    let delay = Duration::from_millis(1);
    interval(delay, scheduler.clone())
      .take(5)
      .subscribe(move |_| (*ticks_c.lock().unwrap()) += 1);
    assert_eq!(0, *ticks.lock().unwrap());
    scheduler.advance(delay * 2);
    scheduler.run_tasks();
    assert_eq!(2, *ticks.lock().unwrap());

    scheduler.advance(delay * 3);
    scheduler.run_tasks();
    assert_eq!(5, *ticks.lock().unwrap());
  }
}
