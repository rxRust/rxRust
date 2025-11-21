use crate::prelude::*;
use std::convert::Infallible;

// Returns an observable which will emit a single `item`
// once after a given `dur` using a given `scheduler`
pub fn timer<Item, S>(
  item: Item,
  dur: Duration,
  scheduler: S,
) -> TimerObservable<Item, S> {
  TimerObservable { item, dur, scheduler }
}

// Returns an observable which will emit a single `item`
// once at a given timestamp `at` using a given `scheduler`.
// If timestamp `at` < `Instant::now()`, the observable will emit the item
// immediately
pub fn timer_at<Item, S>(
  item: Item,
  at: Instant,
  scheduler: S,
) -> TimerObservable<Item, S> {
  let duration = get_duration_from_instant(at);
  TimerObservable { item, dur: duration, scheduler }
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
pub struct TimerObservable<Item, S> {
  item: Item,
  dur: Duration,
  scheduler: S,
}

fn timer_task<Item, Err, O>(
  (mut observer, value): (O, Item),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  observer.next(value);
  observer.complete();
  NormalReturn::new(())
}

impl<Item, O, S> ObservableImpl<Item, Infallible, O>
  for TimerObservable<Item, S>
where
  O: Observer<Item, Infallible>,
  S: Scheduler<OnceTask<(O, Item), NormalReturn<()>>>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { item, dur, scheduler } = self;

    scheduler.schedule(OnceTask::new(timer_task, (observer, item)), Some(dur))
  }
}

impl<Item, S> Observable<Item, Infallible> for TimerObservable<Item, S> {}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
  use std::sync::Arc;

  #[tokio::test]
  async fn timer_shall_emit_value() {
    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer(val, Duration::from_millis(5), LocalScheduler)
        .subscribe(move |n| {
          i_emitted_c.store(n, Ordering::Relaxed);
        });
      local.await;
    }

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn timer_shall_emit_value_shared() {
    let scheduler = SharedScheduler;

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    let (o, status) =
      observable::timer(val, Duration::from_millis(5), scheduler)
        .complete_status();

    o.subscribe(move |n| {
      i_emitted_c.store(n, Ordering::Relaxed);
    });

    status.wait_completed().await;

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[tokio::test]
  async fn timer_shall_call_next_once() {
    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer("aString", Duration::from_millis(5), LocalScheduler)
        .subscribe(move |_| {
          let count = next_count_c.load(Ordering::Relaxed);
          next_count_c.store(count + 1, Ordering::Relaxed);
        });
      local.await;
    }

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn timer_shall_call_next_once_shared() {
    let scheduler = SharedScheduler;

    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    let (o, status) =
      observable::timer("aString", Duration::from_millis(5), scheduler)
        .complete_status();
    o.subscribe(move |_| {
      let count = next_count_c.load(Ordering::Relaxed);
      next_count_c.store(count + 1, Ordering::Relaxed);
    });

    status.wait_completed().await;

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[tokio::test]
  async fn timer_shall_be_completed() {
    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer("aString", Duration::from_millis(5), LocalScheduler)
        .on_complete(move || {
          is_completed_c.store(true, Ordering::Relaxed);
        })
        .subscribe(|_| {});
      local.await;
    }

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn timer_shall_be_completed_shared() {
    let scheduler = SharedScheduler;

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    let (o, status) =
      observable::timer("aString", Duration::from_millis(5), scheduler)
        .on_complete(move || {
          is_completed_c.store(true, Ordering::Relaxed);
        })
        .complete_status();
    let _ = o.subscribe(|_| {});
    status.wait_completed().await;

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[tokio::test]
  async fn timer_shall_elapse_duration() {
    let duration = Duration::from_millis(50);
    let stamp = Instant::now();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer("aString", duration, LocalScheduler).subscribe(|_| {});
      local.await;
    }

    assert!(stamp.elapsed() >= duration);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn timer_shall_elapse_duration_shared() {
    let scheduler = SharedScheduler;

    let duration = Duration::from_millis(50);
    let stamp = Instant::now();

    let (o, status) =
      observable::timer("aString", duration, scheduler).complete_status();
    o.subscribe(|_| {});
    status.wait_completed().await;

    assert!(stamp.elapsed() >= duration);
  }

  #[tokio::test]
  async fn timer_at_shall_emit_value() {
    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer_at(
        val,
        Instant::now() + Duration::from_millis(10),
        LocalScheduler,
      )
      .subscribe(move |n| {
        i_emitted_c.store(n, Ordering::Relaxed);
      });
      local.await;
    }

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn timer_at_shall_emit_value_shared() {
    let scheduler = SharedScheduler;

    let val = 1234;
    let i_emitted = Arc::new(AtomicI32::new(0));
    let i_emitted_c = i_emitted.clone();

    let (o, status) = observable::timer_at(
      val,
      Instant::now() + Duration::from_millis(10),
      scheduler,
    )
    .complete_status();
    o.subscribe(move |n| {
      i_emitted_c.store(n, Ordering::Relaxed);
    });
    status.wait_completed().await;

    assert_eq!(val, i_emitted.load(Ordering::Relaxed));
  }

  #[tokio::test]
  async fn timer_at_shall_call_next_once() {
    let next_count = Arc::new(AtomicUsize::new(0));
    let next_count_c = next_count.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer_at(
        "aString",
        Instant::now() + Duration::from_millis(10),
        LocalScheduler,
      )
      .subscribe(move |_| {
        let count = next_count_c.load(Ordering::Relaxed);
        next_count_c.store(count + 1, Ordering::Relaxed);
      });
      local.await;
    }

    assert_eq!(next_count.load(Ordering::Relaxed), 1);
  }

  #[tokio::test]
  async fn timer_at_shall_be_completed() {
    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer_at(
        "aString",
        Instant::now() + Duration::from_millis(10),
        LocalScheduler,
      )
      .on_complete(move || {
        is_completed_c.store(true, Ordering::Relaxed);
      })
      .subscribe(|_| {});
      local.await;
    }

    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[tokio::test]
  async fn timer_at_shall_elapse_duration_with_valid_timestamp() {
    let duration = Duration::from_millis(50);
    let stamp = Instant::now();
    let execute_at = stamp + duration;

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer_at("aString", execute_at, LocalScheduler)
        .subscribe(|_| {});
      local.await;
    }

    assert!(stamp.elapsed() >= duration);
  }

  #[tokio::test]
  async fn timer_at_shall_complete_with_invalid_timestamp_with_no_delay() {
    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    let duration = Duration::from_secs(1);
    let now = Instant::now();
    let execute_at = now.checked_sub(duration).unwrap(); // execute 1 sec in past

    {
      let local = tokio::task::LocalSet::new();
      let _guard = local.enter();
      observable::timer_at("aString", execute_at, LocalScheduler)
        .on_complete(move || {
          is_completed_c.store(true, Ordering::Relaxed);
        })
        .subscribe(|_| {});
      local.await;
    }

    assert!(now.elapsed() < duration);
    assert!(is_completed.load(Ordering::Relaxed));
  }
}
