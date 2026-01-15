//! Interval Observable Implementation
//!
//! This module provides the `Interval` observable which emits sequential
//! numbers at regular intervals. It's useful for creating periodic tasks,
//! polling mechanisms, or any scenario requiring time-based repeated emissions.
//!
//! The `Interval` observable emits incrementing `usize` values (0, 1, 2, ...)
//! at the specified interval and continues indefinitely until unsubscribed. It
//! never emits an error.
//!
//! ## Timing Behavior
//!
//! The `interval` operator uses **Fixed-Rate** scheduling: it calculates the
//! next emission time based on when the previous emission started, compensating
//! for processing time.
//!
//! - **Fast processing** (processing_time < period): interval ≈ period
//! - **Slow processing** (processing_time >= period): emits immediately after
//!   processing completes (no gap), interval ≈ processing_time
//!
//! This ensures consistent emission timing when processing is faster than the
//! period.
//!
//! ## Examples
//!
//! ```rust,no_run
//! use Duration;
//! use rxrust::prelude::*;
//!
//! // Create an interval that emits every 100ms
//! Local::interval(Duration::from_millis(100))
//!   .take(5) // Take only 5 emissions
//!   .subscribe(|n| println!("Tick {}", n));
//!
//! // Using with Shared scheduler
//! Shared::interval(Duration::from_secs(1)).subscribe(|n| println!("Second: {}", n));
//! ```

// Core dependencies
// Standard library dependencies
use std::convert::Infallible;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Instant, Scheduler, Task, TaskState},
};

/// An observable that emits sequential numbers at regular intervals.
///
/// `Interval` creates an observable sequence that emits incrementing `usize`
/// values (starting from 0) at the specified `period` interval. The emission is
/// scheduled using the provided `scheduler`. The observable continues
/// indefinitely until the observer unsubscribes.
///
/// ## Timing Semantics
///
/// Uses **Fixed-Rate** scheduling: the next emission time is calculated from
/// when the previous emission started, compensating for processing time.
///
/// - Fast processing (< period): interval ≈ period (waits remaining time)
/// - Slow processing (>= period): no gap, interval ≈ processing_time
///
/// This ensures consistent timing when processing is faster than the period.
///
/// # Type Parameters
///
/// * `S` - The scheduler type used to schedule the periodic emissions
///
/// # Fields
///
/// * `period` - The duration between consecutive emissions
/// * `scheduler` - The scheduler used to schedule the emissions
///
/// # Examples
///
/// ```rust,no_run
/// use Duration;
/// use rxrust::prelude::*;
///
/// // Basic interval usage - emits every 50ms
/// Local::interval(Duration::from_millis(50))
///   .take(3) // Limit to 3 emissions
///   .subscribe(|n| println!("Value: {}", n));
///
/// // Interval with SharedScheduler in Local context
/// Local::interval_with(Duration::from_secs(1), SharedScheduler)
///   .subscribe(|n| println!("Second: {}", n));
/// ```
pub struct Interval<S> {
  /// The duration between consecutive emissions
  pub period: Duration,
  /// The scheduler used to schedule the periodic emissions
  pub scheduler: S,
}

/// State for the interval task
struct IntervalState<O> {
  observer: Option<O>,
  counter: usize,
  period: Duration,
}

fn interval_task<O, Err>(state: &mut IntervalState<O>) -> TaskState
where
  O: Observer<usize, Err>,
{
  if let Some(observer) = &mut state.observer
    && !observer.is_closed()
  {
    let scheduled_time = Instant::now();

    observer.next(state.counter);
    state.counter += 1;

    let next_scheduled_time = scheduled_time + state.period;

    // Calculate remaining sleep time based on CURRENT time (after processing)
    let current_time = Instant::now();
    let sleep_duration = if next_scheduled_time > current_time {
      next_scheduled_time - current_time
    } else {
      // Behind schedule, run immediately
      Duration::from_nanos(0)
    };

    return TaskState::Sleeping(sleep_duration);
  }
  TaskState::Finished
}

impl<S> ObservableType for Interval<S> {
  type Item<'a>
    = usize
  where
    Self: 'a;
  type Err = Infallible;
}

impl<S, C> CoreObservable<C> for Interval<S>
where
  C: Context,
  C::Inner: Observer<usize, Infallible>,
  S: Scheduler<Task<IntervalState<C::Inner>>> + Clone,
{
  type Unsub = crate::scheduler::TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let state = IntervalState { observer: Some(observer), counter: 0, period: self.period };

    let task = Task::new(state, interval_task);

    self.scheduler.schedule(task, Some(self.period))
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use super::*;
  use crate::{
    prelude::*,
    scheduler::{Duration, Instant, LocalScheduler, SharedScheduler},
    subscription::Subscription,
  };

  fn create_unsubscribe_task<H: Subscription>(handle: H) -> Task<Option<H>> {
    Task::new(Some(handle), |h| {
      if let Some(h) = h.take() {
        h.unsubscribe();
      }
      TaskState::Finished
    })
  }

  #[rxrust_macro::test(local)]
  async fn test_interval_basic() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    let handle = Local::interval(Duration::from_millis(10)).subscribe(move |v| {
      values_c.lock().unwrap().push(v);
    });

    let unsubscribe_task = create_unsubscribe_task(handle);
    let _scheduled_task =
      LocalScheduler.schedule(unsubscribe_task, Some(Duration::from_millis(65)));
    _scheduled_task.await;

    let result = values.lock().unwrap().clone();
    // Should have received at least 5 values (0, 1, 2, 3, 4)
    assert!(result.len() >= 5, "Expected at least 5 values, got {}", result.len());
    // Verify sequential ordering
    for (i, &val) in result.iter().enumerate() {
      assert_eq!(val, i, "Value at position {} should be {}", i, i);
    }
  }

  #[rxrust_macro::test]
  async fn test_interval_shared() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    let handle = Shared::interval(Duration::from_millis(10)).subscribe(move |v| {
      values_c.lock().unwrap().push(v);
    });

    let unsubscribe_task = create_unsubscribe_task(handle);
    let _scheduled_task =
      SharedScheduler.schedule(unsubscribe_task, Some(Duration::from_millis(65)));

    _scheduled_task.await;

    let result = values.lock().unwrap().clone();
    // Should have received at least 5 values
    assert!(result.len() >= 5, "Expected at least 5 values, got {}", result.len());
    // Verify sequential ordering
    for (i, &val) in result.iter().enumerate() {
      assert_eq!(val, i, "Shared interval value at position {} should be {}", i, i);
    }
  }

  #[rxrust_macro::test(local)]
  async fn test_interval_timing() {
    let start_time = Instant::now();
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    let handle = Local::interval(Duration::from_millis(20)).subscribe(move |v| {
      values_c.lock().unwrap().push(v);
    });

    let unsubscribe_task = create_unsubscribe_task(handle);
    let _scheduled_task =
      LocalScheduler.schedule(unsubscribe_task, Some(Duration::from_millis(80)));
    _scheduled_task.await;

    let elapsed_time = start_time.elapsed();
    let result = values.lock().unwrap().clone();

    // Should have received at least 3 values
    assert!(result.len() >= 3, "Expected at least 3 values in 80ms, got {}", result.len());
    for (i, &val) in result.iter().enumerate() {
      assert_eq!(val, i, "Timing test value at position {} should be {}", i, i);
    }

    // Should have taken at least 60ms (3 intervals of 20ms)
    assert!(
      elapsed_time >= Duration::from_millis(60),
      "Expected elapsed time >= 60ms, got {:?}",
      elapsed_time
    );
  }

  #[rxrust_macro::test]
  async fn test_interval_unsubscribe() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    let handle = Shared::interval(Duration::from_millis(10)).subscribe(move |v| {
      values_c.lock().unwrap().push(v);
    });

    let cancel_interval = create_unsubscribe_task(handle);
    let handle = SharedScheduler.schedule(cancel_interval, Some(Duration::from_millis(35)));

    handle.await;

    let count_at_unsub = values.lock().unwrap().len();

    let wati_50_mills = SharedScheduler
      .schedule(Task::new((), |_| TaskState::Finished), Some(Duration::from_millis(50)));

    wati_50_mills.await;

    // Should not have received more values after unsubscribe
    assert_eq!(values.lock().unwrap().len(), count_at_unsub);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test(local)]
  async fn test_interval_fixed_rate_behavior() {
    // Test configuration
    let interval_period = Duration::from_millis(20);
    let slow_processing = Duration::from_millis(30); // > interval_period
    let fast_processing = Duration::from_millis(5); // < interval_period

    // Track emission times
    let emission_times = Arc::new(Mutex::new(Vec::new()));
    let times_clone = emission_times.clone();
    let test_start = Instant::now();

    // Create interval with mixed processing times:
    // - First 2 emissions: slow processing (30ms)
    // - Remaining emissions: fast processing (5ms)
    let handle = Local::interval(interval_period).subscribe(move |value| {
      let emission_time = test_start.elapsed();
      times_clone
        .lock()
        .unwrap()
        .push((value, emission_time));

      let processing_time = if value < 2 { slow_processing } else { fast_processing };
      std::thread::sleep(processing_time);
    });

    // Run test long enough to observe both phases
    let unsubscribe_task = create_unsubscribe_task(handle);
    let _scheduled_task =
      LocalScheduler.schedule(unsubscribe_task, Some(Duration::from_millis(200)));
    _scheduled_task.await;

    let emissions = emission_times.lock().unwrap().clone();

    // Verify we captured enough emissions
    assert!(
      emissions.len() >= 4,
      "Need at least 4 emissions to test behavior, got {}",
      emissions.len()
    );

    // Extract timing intervals between consecutive emissions
    let intervals: Vec<Duration> = emissions
      .windows(2)
      .map(|pair| pair[1].1 - pair[0].1)
      .collect();

    // With Fixed-Rate scheduling:
    // - When processing_time < period: interval ≈ period (waits remaining time)
    // - When processing_time >= period: interval ≈ processing_time (no wait)
    let tolerance = Duration::from_millis(10);

    // Phase 1: Slow processing (emissions 0->1, 1->2)
    // Processing (30ms) >= Period (20ms), so next runs immediately after processing
    // Expected interval ≈ processing_time (30ms), NOT processing_time + period
    let expected_slow_interval = slow_processing;

    // After emission 0 (Slow) -> emission 1
    assert!(
      intervals[0] >= expected_slow_interval - tolerance,
      "Slow phase 1: interval should be ~processing_time. Expected ~{:?}, got {:?}",
      expected_slow_interval,
      intervals[0]
    );
    assert!(
      intervals[0] < slow_processing + interval_period,
      "Slow phase 1: should NOT add full period on top of processing. Got {:?}",
      intervals[0]
    );

    // After emission 1 (Slow) -> emission 2
    assert!(
      intervals[1] >= expected_slow_interval - tolerance,
      "Slow phase 2: interval should be ~processing_time. Expected ~{:?}, got {:?}",
      expected_slow_interval,
      intervals[1]
    );

    // Phase 2: Transition to fast processing (emission 2->3)
    // Processing (5ms) < Period (20ms), so waits remaining time (15ms)
    // Expected interval ≈ period (20ms)
    let expected_fast_interval = interval_period;

    assert!(
      intervals[2] >= expected_fast_interval - tolerance,
      "Fast phase: interval should be ~period. Expected ~{:?}, got {:?}",
      expected_fast_interval,
      intervals[2]
    );
    assert!(
      intervals[2] < interval_period + tolerance,
      "Fast phase: interval should be close to period. Got {:?}",
      intervals[2]
    );

    // Phase 3: Continued Fast processing (emission 3->4)
    if intervals.len() > 3 {
      assert!(
        intervals[3] >= expected_fast_interval - tolerance,
        "Fast phase continued: expected ~{:?}, got {:?}",
        expected_fast_interval,
        intervals[3]
      );
    }
  }
}
