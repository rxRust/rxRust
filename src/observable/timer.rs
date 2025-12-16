//! Timer Observable Implementation
//!
//! This module provides the `Timer` observable which emits a single value after
//! a specified delay. It's useful for introducing time-based delays, creating
//! timeout mechanisms, or scheduling work to be executed after a specific
//! duration.
//!
//! The `Timer` observable emits a single value after the specified delay
//! and then completes. It never emits an error.
//!
//! ## Examples
//!
//! ```rust,no_run
//! use rxrust::prelude::*;
//!
//! // Create a timer that emits after 100ms
//! Local::timer(Duration::from_millis(100)).subscribe(|_| println!("Timer fired!"));
//! ```
// Standard library imports
use std::convert::Infallible;

// Internal module imports
use crate::context::Context;
use crate::{
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskState},
};

/// An observable that emits `()` after a specified delay.
///
/// `Timer` creates an observable sequence that emits a single `()` after the
/// specified `delay`, then completes. The emission is scheduled using the
/// provided `scheduler`.
///
/// # Type Parameters
///
/// * `S` - The scheduler type used to schedule the delayed emission
///
/// # Fields
///
/// * `delay` - The duration to wait before emitting
/// * `scheduler` - The scheduler used to schedule the emission
///
/// # Examples
///
/// ```rust,no_run
/// use rxrust::prelude::*;
///
/// // Basic timer usage
/// Local::timer(Duration::from_millis(50)).subscribe(|_| println!("50ms have passed"));
///
/// // Timer with SharedScheduler in Local context
/// Local::timer_with(Duration::from_secs(1), SharedScheduler)
///   .subscribe(|_| println!("Timer fired with SharedScheduler"));
/// ```
pub struct Timer<S> {
  /// The duration to wait before emitting the value
  pub delay: Duration,
  /// The scheduler used to schedule the delayed emission
  pub scheduler: S,
}

fn timer_task<O, Err>((observer, _): &mut (Option<O>, ())) -> TaskState
where
  O: Observer<(), Err>,
{
  if let Some(mut observer) = observer.take() {
    observer.next(());
    observer.complete();
  }
  TaskState::Finished
}

impl<S> ObservableType for Timer<S> {
  type Item<'a>
    = ()
  where
    Self: 'a;
  type Err = Infallible;
}

impl<S, C> CoreObservable<C> for Timer<S>
where
  C: Context,
  C::Inner: Observer<(), Infallible>,
  S: Scheduler<Task<(Option<C::Inner>, ())>> + Clone,
{
  type Unsub = crate::scheduler::TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let task = Task::new((Some(observer), ()), timer_task);
    self.scheduler.schedule(task, Some(self.delay))
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_timer_basic() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    Local::timer(Duration::from_millis(10))
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    // Wait for timer
    LocalScheduler
      .sleep(Duration::from_millis(20))
      .await;

    assert_eq!(*value.lock().unwrap(), Some(()));
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test]
  async fn test_timer_shared() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    let handle = Shared::timer(Duration::from_millis(10))
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    // Wait for timer
    handle.await;

    assert_eq!(*value.lock().unwrap(), Some(()));
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_timer_duration() {
    let start = Instant::now();
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();

    Local::timer(Duration::from_millis(50)).subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(60))
      .await;

    assert!(start.elapsed() >= Duration::from_millis(50));
    assert_eq!(*value.lock().unwrap(), Some(()));
  }

  #[rxrust_macro::test(local)]
  async fn test_timer_emit_value() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();

    // Use map instead of timer_emit
    Local::timer(Duration::from_millis(10))
      .map(|_| 123)
      .subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(20))
      .await;

    assert_eq!(*value.lock().unwrap(), Some(123));
  }

  #[rxrust_macro::test(local)]
  async fn test_timer_at_basic() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();
    let now = Instant::now();
    let target = now + Duration::from_millis(10);

    Local::timer_at(target).subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(20))
      .await;

    assert_eq!(*value.lock().unwrap(), Some(()));
    assert!(Instant::now() >= target);
  }

  #[rxrust_macro::test(local)]
  async fn test_timer_at_emit_value() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();
    let target = Instant::now() + Duration::from_millis(10);

    // Use map instead of timer_at_emit
    Local::timer_at(target)
      .map(|_| "hello")
      .subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(20))
      .await;

    assert_eq!(*value.lock().unwrap(), Some("hello"));
  }

  #[rxrust_macro::test(local)]
  async fn test_timer_at_past_time() {
    let value = Arc::new(Mutex::new(None));
    let value_c = value.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    // Time in the past (or present which is treated same)
    let target = Instant::now();

    Local::timer_at(target)
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| *value_c.lock().unwrap() = Some(v));

    // Should fire immediately (on next tick)
    LocalScheduler
      .sleep(Duration::from_millis(5))
      .await;

    assert_eq!(*value.lock().unwrap(), Some(()));
    assert!(*completed.lock().unwrap());
  }
}
