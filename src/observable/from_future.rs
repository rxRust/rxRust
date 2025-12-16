//! FromFuture observable implementation
//!
//! This module provides the `FromFuture` and `FromFutureResult` observables
//! which convert a `Future` into an Observable that emits the future's result
//! and completes.
//!
//! ## Examples
//!
//! ```rust,no_run
//! use std::future;
//!
//! use rxrust::prelude::*;
//!
//! // Create an observable from a future
//! Local::from_future(future::ready(42)).subscribe(|v| println!("Got: {}", v));
//!
//! // Create an observable from a future that returns Result
//! // Ok values are emitted via next(), Err values via error()
//! Local::from_future_result(future::ready(Ok::<_, String>(42)))
//!   .on_error(|e| println!("Error: {}", e))
//!   .subscribe(|v| println!("Got: {}", v));
//! ```
//!
//! ## Note on Scheduler Integration
//!
//! The `from_future` operator requires a scheduler that supports spawning async
//! tasks. It works differently from synchronous operators - instead of using
//! `Task<S>` directly, it uses `Task::new` with a wrapper state that will be
//! processed by the scheduler's async runtime.

// Standard library imports
use std::{
  convert::Infallible,
  future::Future,
  pin::Pin,
  task::{Context as TaskContext, Poll},
};

// External crate imports
use pin_project_lite::pin_project;

// Internal module imports
use crate::context::Context;
use crate::{
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Scheduler, TaskHandle},
};

/// An observable that converts a `Future` into an Observable sequence.
///
/// `FromFuture` creates an observable that, upon subscription, spawns the
/// future on the provided scheduler. When the future completes, it emits the
/// result as a single value, then completes.
///
/// # Type Parameters
///
/// * `F` - The future type
/// * `S` - The scheduler type used to spawn the async task
///
/// # Note
///
/// This operator leverages the scheduler's native support for Futures.
/// The future is executed asynchronously according to the scheduler's policy
/// (e.g., `LocalScheduler` for local execution, `SharedScheduler` for thread
/// pools).
///
/// # Examples
///
/// ```rust,no_run
/// use std::future;
///
/// use rxrust::prelude::*;
///
/// Local::from_future(future::ready("hello")).subscribe(|v| println!("Got: {}", v));
/// ```
#[derive(Clone)]
pub struct FromFuture<F, S> {
  /// The future to convert
  pub future: F,
  /// The scheduler used to spawn the task
  pub scheduler: S,
}

impl<F: Future, S> ObservableType for FromFuture<F, S> {
  type Item<'a>
    = F::Output
  where
    Self: 'a;
  type Err = Infallible;
}

pin_project! {
  /// A future that drives the inner future and emits the result to the observer.
  pub struct FromFutureTask<F, O> {
    #[pin]
    future: F,
    observer: Option<O>,
  }
}

impl<F, O> Future for FromFutureTask<F, O>
where
  F: Future,
  O: Observer<F::Output, Infallible>,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match this.future.poll(cx) {
      Poll::Ready(value) => {
        if let Some(mut observer) = this.observer.take() {
          observer.next(value);
          observer.complete();
        }
        Poll::Ready(())
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

impl<F, C, S> CoreObservable<C> for FromFuture<F, S>
where
  C: Context,
  C::Inner: Observer<F::Output, Infallible>,
  F: Future,
  S: Scheduler<FromFutureTask<F, C::Inner>> + Clone,
{
  type Unsub = TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let task = FromFutureTask { future: self.future, observer: Some(observer) };
    self.scheduler.schedule(task, None)
  }
}

// ============================================================================
// FromFutureResult - For futures that return Result types
// ============================================================================

/// An observable that converts a `Future<Output = Result<Item, Err>>` into an
/// Observable sequence.
///
/// Unlike `FromFuture`, this observable handles `Result` types specially:
/// - `Ok(value)` is emitted via `next()` followed by `complete()`
/// - `Err(error)` is emitted via `error()`
///
/// # Type Parameters
///
/// * `F` - The future type that outputs a `Result`
/// * `S` - The scheduler type used to spawn the async task
///
/// # Examples
///
/// ```rust,no_run
/// use std::future;
///
/// use rxrust::prelude::*;
///
/// // Success case
/// Local::from_future_result(future::ready(Ok::<_, String>(42)))
///   .on_error(|_e| {})
///   .subscribe(|v| println!("Got: {}", v));
///
/// // Error case
/// Local::from_future_result(future::ready(Err::<i32, _>("error")))
///   .on_error(|e| println!("Error: {}", e))
///   .subscribe(|v| println!("Got: {}", v));
/// ```
#[derive(Clone)]
pub struct FromFutureResult<F, S> {
  /// The future to convert (must output a Result type)
  pub future: F,
  /// The scheduler used to spawn the task
  pub scheduler: S,
}

impl<F, Item, Err, S> ObservableType for FromFutureResult<F, S>
where
  F: Future<Output = Result<Item, Err>>,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Err;
}

pin_project! {
  /// A future that drives the inner future and emits the result to the observer,
  /// handling Result types by routing Ok to next() and Err to error().
  pub struct FromFutureResultTask<F, O> {
    #[pin]
    future: F,
    observer: Option<O>,
  }
}

impl<F, O, Item, Err> Future for FromFutureResultTask<F, O>
where
  F: Future<Output = Result<Item, Err>>,
  O: Observer<Item, Err>,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match this.future.poll(cx) {
      Poll::Ready(result) => {
        if let Some(mut observer) = this.observer.take() {
          match result {
            Ok(value) => {
              observer.next(value);
              observer.complete();
            }
            Err(err) => {
              observer.error(err);
            }
          }
        }
        Poll::Ready(())
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

impl<F, C, S, Item, Err> CoreObservable<C> for FromFutureResult<F, S>
where
  C: Context,
  C::Inner: Observer<Item, Err>,
  F: Future<Output = Result<Item, Err>>,
  S: Scheduler<FromFutureResultTask<F, C::Inner>> + Clone,
{
  type Unsub = TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let task = FromFutureResultTask { future: self.future, observer: Some(observer) };
    self.scheduler.schedule(task, None)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    future,
    sync::{Arc, Mutex},
  };

  use crate::{
    prelude::*,
    scheduler::{Duration, LocalScheduler, SleepProvider},
  };

  #[rxrust_macro::test(local)]
  async fn test_from_future_ready() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    Local::from_future(future::ready(42))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| {
        *result_clone.lock().unwrap() = Some(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), Some(42));
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_future_async() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    Local::from_future(future::ready(42))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| {
        *result_clone.lock().unwrap() = Some(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), Some(42));
    assert!(*completed.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_future_shared() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Shared::from_future(future::ready("hello"))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| {
        *result_clone.lock().unwrap() = Some(v);
      });

    handle.await;

    assert_eq!(*result.lock().unwrap(), Some("hello"));
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_future_with_map() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    Local::from_future(future::ready(10))
      .map(|v| v * 2)
      .subscribe(move |v| {
        *result_clone.lock().unwrap() = Some(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), Some(20));
  }

  // ============================================================================
  // FromFutureResult tests
  // ============================================================================

  #[rxrust_macro::test(local)]
  async fn test_from_future_result_ok() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(false));
    let error_clone = error_received.clone();

    Local::from_future_result(future::ready(Ok::<_, String>(42)))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |_| *error_clone.lock().unwrap() = true)
      .subscribe(move |v| {
        *result_clone.lock().unwrap() = Some(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), Some(42));
    assert!(*completed.lock().unwrap());
    assert!(!*error_received.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_future_result_err() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    Local::from_future_result(future::ready(Err::<i32, _>("test error".to_string())))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| *result_clone.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), None);
    assert!(!*completed.lock().unwrap());
    assert_eq!(*error_received.lock().unwrap(), Some("test error".to_string()));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_future_result_shared_ok() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(false));
    let error_clone = error_received.clone();

    let handle = Shared::from_future_result(future::ready(Ok::<_, String>("success")))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |_| *error_clone.lock().unwrap() = true)
      .subscribe(move |v| *result_clone.lock().unwrap() = Some(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), Some("success"));
    assert!(*completed.lock().unwrap());
    assert!(!*error_received.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_future_result_shared_err() {
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Shared::from_future_result(future::ready(Err::<i32, _>("shared error")))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(|_| {});

    handle.await;

    assert!(!*completed.lock().unwrap());
    assert_eq!(*error_received.lock().unwrap(), Some("shared error"));
  }

  #[rxrust_macro::test(local)]
  async fn test_from_future_result_with_map() {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    Local::from_future_result(future::ready(Ok::<_, String>(10)))
      .map(|v| v * 2)
      .on_error(|_| {})
      .subscribe(move |v| *result_clone.lock().unwrap() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.lock().unwrap(), Some(20));
  }
}
