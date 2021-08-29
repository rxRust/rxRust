use crate::prelude::*;
use std::time::{Duration, Instant};

pub fn timer<Item, S>(item: Item, dur: Duration, scheduler: S) -> ObservableBase<TimerEmitter<Item, S>> {
  ObservableBase::new(TimerEmitter {
    item,
    dur: Some(dur),
    at: None,
    scheduler,
  })
}

pub fn timer_at<Item, S>(item: Item, at: Instant, scheduler: S) -> ObservableBase<TimerEmitter<Item, S>> {
  ObservableBase::new(TimerEmitter {
    item,
    dur: None,
    at: Some(at),
    scheduler,
  })
}

pub struct TimerEmitter<Item, S> {
  item: Item,
  dur: Option<Duration>,
  at: Option<Instant>,
  scheduler: S,
}

impl<Item, S> Emitter for TimerEmitter<Item, S> {
  type Item = Item;
  type Err = ();
}

impl<Item: 'static, S: LocalScheduler + 'static> LocalEmitter<'static> for TimerEmitter<Item, S> {
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>) where O: Observer<Item=Self::Item, Err=Self::Err> + 'static {
    let mut observer = subscriber.observer;
    let item = self.item;

    let dur = if let Some(duration) = self.dur {
      duration
    } else if let Some(at) = self.at {
      let now = Instant::now();
      if at > now {
        at - now
      } else {
        Duration::default()
      }
    } else {
      Duration::default()
    };

    let handle = self.scheduler.schedule(
      move |_| {
        observer.next(item);
        observer.complete();
      },
      Some(dur),
      1);

    subscriber.subscription.add(handle);
  }
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use futures::executor::LocalPool;
  use std::time::{Duration, Instant};
  use std::sync::atomic::{AtomicBool, Ordering, AtomicUsize};
  use std::sync::Arc;

  #[test]
  fn timer_shall_call_next_once() {
    let mut local = LocalPool::new();

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    observable::timer("aString", Duration::from_millis(5), local.spawner())
        .subscribe(
          move |_| {
            let count = next_count_c.load(Ordering::Relaxed);
            next_count_c.store(count + 1, Ordering::Relaxed);
          }
        );

    local.run();

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

    assert_eq!(is_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn timer_shall_elapse_duration() {
    let mut local = LocalPool::new();

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();

    observable::timer("aString", duration.clone(), local.spawner())
        .subscribe(
          |_| {}
        );

    local.run();

    assert!(stamp.elapsed() >= duration);
  }

  #[test]
  fn timer_at_shall_call_next_once() {
    let mut local = LocalPool::new();

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    observable::timer_at("aString", Instant::now() + Duration::from_millis(10), local.spawner())
        .subscribe(
          move |_| {
            let count = next_count_c.load(Ordering::Relaxed);
            next_count_c.store(count + 1, Ordering::Relaxed);
          }
        );

    local.run();

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn timer_at_shall_be_completed() {
    let mut local = LocalPool::new();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::timer_at("aString", Instant::now() + Duration::from_millis(10), local.spawner())
        .subscribe_complete(
          |_| {},
          move || {
            is_completed_c.store(true, Ordering::Relaxed);
          },
        );

    local.run();

    assert_eq!(is_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn timer_at_shall_elapse_duration_with_valid_timestamp() {
    let mut local = LocalPool::new();

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();
    let execute_at = stamp + duration;

    observable::timer_at("aString", execute_at, local.spawner())
        .subscribe(
          |_| {}
        );

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
    assert_eq!(is_completed.load(Ordering::Relaxed), true);
  }
}
