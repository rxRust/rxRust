use crate::prelude::*;
use std::time::{Duration, Instant};

// Returns an observable which will emit a single `item`
// once after a given `dur` using a given `scheduler`
pub fn timer<Item, S>(
  item: Item,
  dur: Duration,
  scheduler: S,
) -> ObservableBase<TimerEmitter<Item, S>> {
  ObservableBase::new(TimerEmitter {
    item,
    dur,
    scheduler,
  })
}

// Returns an observable which will emit a single `item`
// once at a given timestamp `at` using a given `scheduler`.
// If timestamp `at` < `Instant::now()`, the observable will emit the item
// immediately
pub fn timer_at<Item, S>(
  item: Item,
  at: Instant,
  scheduler: S,
) -> ObservableBase<TimerEmitter<Item, S>> {
  let duration = get_duration_from_instant(at);
  ObservableBase::new(TimerEmitter {
    item,
    dur: duration,
    scheduler,
  })
}

// Calculates the duration between `Instant::now()` and a given `instant`.
// Returns `Duration::default()` when `instant` is a timestamp in the past
fn get_duration_from_instant(instant: Instant) -> Duration {
  let now = Instant::now();
  match instant > now {
    true => instant - now,
    false => Duration::default(),
  }
}

// Emitter for `observable::timer` and `observable::timer_at` holding the
// `item` that will be emitted, a `dur` when this will happen and the used
// `scheduler`
pub struct TimerEmitter<Item, S> {
  item: Item,
  dur: Duration,
  scheduler: S,
}

impl<Item, S> Emitter for TimerEmitter<Item, S> {
  type Item = Item;
  type Err = ();
}

impl<Item: 'static, S: LocalScheduler + 'static> LocalEmitter<'static>
  for TimerEmitter<Item, S>
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    let item = self.item;
    let dur = self.dur;

    self.scheduler.schedule(
      move |_| {
        observer.next(item);
        observer.complete();
      },
      Some(dur),
      1,
    )
  }
}

impl<Item: Send + 'static, S: SharedScheduler + 'static> SharedEmitter
  for TimerEmitter<Item, S>
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    let item = self.item;
    let dur = self.dur;

    self.scheduler.schedule(
      move |_| {
        observer.next(item);
        observer.complete();
      },
      Some(dur),
      1,
    )
  }
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
  use std::sync::Arc;
  use std::time::{Duration, Instant};

  #[test]
  fn timer_shall_emit_value() {
    let mut local = LocalPool::new();

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    observable::timer(val, Duration::from_millis(5), local.spawner())
      .subscribe(move |n| {
        i_emitted_c.store(n, Ordering::Relaxed);
      });

    local.run();

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_shall_emit_value_shared() {
    let pool = ThreadPool::new().unwrap();

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    observable::timer(val, Duration::from_millis(5), pool)
      .into_shared()
      .subscribe_blocking(move |n| {
        i_emitted_c.store(n, Ordering::Relaxed);
      });

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_shall_call_next_once() {
    let mut local = LocalPool::new();

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    observable::timer("aString", Duration::from_millis(5), local.spawner())
      .subscribe(move |_| {
        let count = next_count_c.load(Ordering::Relaxed);
        next_count_c.store(count + 1, Ordering::Relaxed);
      });

    local.run();

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn timer_shall_call_next_once_shared() {
    let pool = ThreadPool::new().unwrap();

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    observable::timer("aString", Duration::from_millis(5), pool)
      .into_shared()
      .subscribe_blocking(move |_| {
        let count = next_count_c.load(Ordering::Relaxed);
        next_count_c.store(count + 1, Ordering::Relaxed);
      });

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn timer_shall_be_completed() {
    let mut local = LocalPool::new();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::timer("aString", Duration::from_millis(5), local.spawner())
      .subscribe_complete(
        |_| {},
        move || {
          is_completed_c.store(true, Ordering::Relaxed);
        },
      );

    local.run();

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_shall_be_completed_shared() {
    let pool = ThreadPool::new().unwrap();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::timer("aString", Duration::from_millis(5), pool)
      .into_shared()
      .subscribe_blocking_all(
        |_| {},
        |_| {},
        move || {
          is_completed_c.store(true, Ordering::Relaxed);
        },
      );

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_shall_elapse_duration() {
    let mut local = LocalPool::new();

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();

    observable::timer("aString", duration, local.spawner()).subscribe(|_| {});

    local.run();

    assert!(stamp.elapsed() >= duration);
  }

  #[test]
  fn timer_shall_elapse_duration_shared() {
    let pool = ThreadPool::new().unwrap();

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();

    observable::timer("aString", duration, pool)
      .into_shared()
      .subscribe_blocking(|_| {});

    assert!(stamp.elapsed() >= duration);
  }

  #[test]
  fn timer_at_shall_emit_value() {
    let mut local = LocalPool::new();

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    observable::timer_at(
      val,
      Instant::now() + Duration::from_millis(10),
      local.spawner(),
    )
    .subscribe(move |n| {
      i_emitted_c.store(n, Ordering::Relaxed);
    });

    local.run();

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_at_shall_emit_value_shared() {
    let pool = ThreadPool::new().unwrap();

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    observable::timer_at(val, Instant::now() + Duration::from_millis(10), pool)
      .into_shared()
      .subscribe_blocking(move |n| {
        i_emitted_c.store(n, Ordering::Relaxed);
      });

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_at_shall_call_next_once() {
    let mut local = LocalPool::new();

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    observable::timer_at(
      "aString",
      Instant::now() + Duration::from_millis(10),
      local.spawner(),
    )
    .subscribe(move |_| {
      let count = next_count_c.load(Ordering::Relaxed);
      next_count_c.store(count + 1, Ordering::Relaxed);
    });

    local.run();

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn timer_at_shall_be_completed() {
    let mut local = LocalPool::new();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::timer_at(
      "aString",
      Instant::now() + Duration::from_millis(10),
      local.spawner(),
    )
    .subscribe_complete(
      |_| {},
      move || {
        is_completed_c.store(true, Ordering::Relaxed);
      },
    );

    local.run();

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn timer_at_shall_elapse_duration_with_valid_timestamp() {
    let mut local = LocalPool::new();

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();
    let execute_at = stamp + duration;

    observable::timer_at("aString", execute_at, local.spawner())
      .subscribe(|_| {});

    local.run();

    assert!(stamp.elapsed() >= duration);
  }

  #[test]
  fn timer_at_shall_complete_with_invalid_timestamp_with_no_delay() {
    let mut local = LocalPool::new();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    let duration = Duration::from_secs(1);
    let now = Instant::now();
    let execute_at = now - duration; // execute 1 sec in past

    observable::timer_at("aString", execute_at, local.spawner())
      .subscribe_complete(
        |_| {},
        move || {
          is_completed_c.store(true, Ordering::Relaxed);
        },
      );

    local.run();

    assert!(now.elapsed() < duration);
    assert!(is_completed.load(Ordering::Relaxed));
  }
}
