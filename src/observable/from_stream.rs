//! FromStream observable implementation
//!
//! This module provides the `FromStream` observable which converts a
//! `futures_core::stream::Stream` into an Observable that emits each item from
//! the stream and then completes.
//!
//! ## Direct futures_core::stream::Stream Support
//!
//! This implementation directly accepts `futures_core::stream::Stream` for
//! simplicity and ecosystem compatibility. The `futures_core::stream::Stream`
//! trait is the standard async iterator in Rust:
//!
//! ```rust,ignore
//! pub trait Stream {
//!     type Item;
//!     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
//! }
//! ```
//!
//! Key semantics:
//! - `Poll::Ready(Some(value))` - A value is available, can poll again for more
//! - `Poll::Ready(None)` - Stream exhausted, no more values
//! - `Poll::Pending` - No value yet, will wake when ready
//!
//! ## Scheduler Compatibility
//!
//! `FromStream` works seamlessly with all scheduler types through generic trait
//! bounds:
//!
//! - **LocalScheduler**: Single-threaded execution, no `Send` requirement
//! - **SharedScheduler**: Multi-threaded execution with `Send` bounds
//! - **Custom schedulers**: Any type implementing the `Scheduler` trait
//!
//! ## Examples
//!
//! ```rust,no_run
//! use futures::stream;
//! use rxrust::prelude::*;
//!
//! // Create an observable from a futures_core::Stream
//! let stream = stream::iter(vec![1, 2, 3]);
//! Local::from_stream(stream)
//!   .map(|x| x * 2)
//!   .subscribe(|v| println!("{}", v));
//! // Emits: 2, 4, 6, then completes
//! ```
//!
//! ## Error Handling and Cancellation
//!
//! - **Cancellation**: Unsubscribing stops polling the stream and cleans up
//!   resources
//! - **Panic safety**: The scheduler's `CatchUnwind` wrapper prevents panic
//!   propagation
//! - **Resource cleanup**: Observers are properly dropped on completion or
//!   cancellation

use std::{
  convert::Infallible,
  future::Future,
  pin::Pin,
  task::{Context as TaskContext, Poll},
};

use futures_core::stream::Stream;
use pin_project_lite::pin_project;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::Scheduler,
};

/// An observable that converts a `futures_core::stream::Stream` into an
/// Observable sequence.
///
/// `FromStream` creates an observable that, upon subscription, polls the stream
/// and emits each item. When the stream is exhausted (returns `Ready(None)`),
/// the observable completes.
///
/// # Type Parameters
///
/// * `St` - The stream type implementing `futures_core::stream::Stream`
/// * `S` - The scheduler type used to spawn the async task
///
/// # Polling Behavior
///
/// When subscribed, `FromStream` creates a `FromStreamTask` future that:
/// 1. Polls the stream using `poll_next()`
/// 2. On `Ready(Some(value))`: emit value, continue polling
/// 3. On `Ready(None)`: stream exhausted, complete
/// 4. On `Pending`: yield control, scheduler will retry
#[derive(Clone)]
pub struct FromStream<St, S> {
  /// The stream to convert
  pub stream: St,
  /// The scheduler used to spawn the task
  pub scheduler: S,
}

impl<St, S> ObservableType for FromStream<St, S>
where
  St: Stream,
{
  type Item<'a>
    = St::Item
  where
    Self: 'a;
  type Err = Infallible;
}

pin_project! {
  /// A future that drives the stream and emits values to the observer.
  ///
  /// `FromStreamTask` implements the core polling logic for `futures_core::stream::Stream`.
  ///
  /// # Polling Algorithm
  ///
  /// 1. **Check cancellation**: If observer is None, return Ready(()) immediately
  /// 2. **Poll stream**: Call `poll_next()` on the stream
  ///    - If `Ready(Some(value))`: emit value, continue loop
  ///    - If `Ready(None)`: stream exhausted, complete observer, return Ready(())
  ///    - If `Pending`: return Pending (yield control to scheduler)
  ///
  /// # Type Parameters
  ///
  /// * `St` - The stream type implementing `futures_core::stream::Stream`
  /// * `O` - The observer type that receives emitted values
  pub struct FromStreamTask<St, O> {
    #[pin]
    stream: St,
    observer: Option<O>,
  }
}

impl<St, O> Future for FromStreamTask<St, O>
where
  St: Stream,
  O: Observer<St::Item, Infallible>,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let mut this = self.project();

    loop {
      // Check if observer has been dropped (cancellation)
      if this.observer.is_none() {
        return Poll::Ready(());
      }

      // Poll the stream
      match this.stream.as_mut().poll_next(cx) {
        Poll::Ready(Some(value)) => {
          // Emit value to observer
          if let Some(observer) = this.observer.as_mut() {
            observer.next(value);
          }
          // Continue loop to poll for next value immediately
        }
        Poll::Ready(None) => {
          // Stream exhausted, complete
          if let Some(observer) = this.observer.take() {
            observer.complete();
          }
          return Poll::Ready(());
        }
        Poll::Pending => {
          // No value available yet, yield control
          return Poll::Pending;
        }
      }
    }
  }
}

/// Unified generic implementation that works with any scheduler type.
///
/// # Requirements Satisfied
///
/// - **1.1**: Creates Observable from `futures_core::stream::Stream`
/// - **1.4**: Generic scheduler integration with trait bounds
/// - **2.1**: LocalScheduler support without Send requirements
/// - **2.2**: SharedScheduler support with automatic Send enforcement
impl<St, S, C> CoreObservable<C> for FromStream<St, S>
where
  C: Context,
  C::Inner: Observer<St::Item, Infallible>,
  St: Stream,
  S: Scheduler<FromStreamTask<St, C::Inner>> + Clone,
{
  type Unsub = crate::scheduler::TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let task = FromStreamTask { stream: self.stream, observer: Some(observer) };
    self.scheduler.schedule(task, None)
  }
}

// ============================================================================
// FromStreamResult - For streams that emit Result types
// ============================================================================

/// An observable that converts a `Stream<Item = Result<Item, Err>>` into an
/// Observable sequence.
///
/// Unlike `FromStream`, this observable handles `Result` types specially:
/// - `Ok(value)` is emitted via `next()`
/// - `Err(error)` is emitted via `error()` and terminates the stream
///
/// # Type Parameters
///
/// * `St` - The stream type that outputs `Result<Item, Err>`
/// * `S` - The scheduler type used to spawn the async task
///
/// # Examples
///
/// ```rust,no_run
/// use std::convert::Infallible;
///
/// use futures::stream;
/// use rxrust::prelude::*;
///
/// // Success case - emits values and completes
/// let stream = stream::iter(vec![Ok::<i32, Infallible>(1), Ok(2), Ok(3)]);
/// Local::from_stream_result(stream)
///   .on_error(|_e| {})
///   .subscribe(|v| println!("Got: {}", v));
///
/// // Error case - emits values until error, then terminates
/// let stream = stream::iter(vec![Ok(1), Err("error"), Ok(3)]);
/// Local::from_stream_result(stream)
///   .on_error(|e| println!("Error: {}", e))
///   .subscribe(|v| println!("Got: {}", v));
/// // Output: Got: 1, Error: error
/// ```
#[derive(Clone)]
pub struct FromStreamResult<St, S> {
  /// The stream to convert (must output Result types)
  pub stream: St,
  /// The scheduler used to spawn the task
  pub scheduler: S,
}

impl<St, Item, Err, S> ObservableType for FromStreamResult<St, S>
where
  St: Stream<Item = Result<Item, Err>>,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Err;
}

pin_project! {
  /// A future that drives the stream and emits values to the observer,
  /// handling Result types by routing Ok to next() and Err to error().
  pub struct FromStreamResultTask<St, O> {
    #[pin]
    stream: St,
    observer: Option<O>,
  }
}

impl<St, O, Item, Err> Future for FromStreamResultTask<St, O>
where
  St: Stream<Item = Result<Item, Err>>,
  O: Observer<Item, Err>,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let mut this = self.project();

    loop {
      // Check if observer has been dropped (cancellation)
      if this.observer.is_none() {
        return Poll::Ready(());
      }

      // Poll the stream
      match this.stream.as_mut().poll_next(cx) {
        Poll::Ready(Some(result)) => {
          match result {
            Ok(value) => {
              // Emit value to observer
              if let Some(observer) = this.observer.as_mut() {
                observer.next(value);
              }
              // Continue loop to poll for next value immediately
            }
            Err(err) => {
              // Emit error and terminate
              if let Some(observer) = this.observer.take() {
                observer.error(err);
              }
              return Poll::Ready(());
            }
          }
        }
        Poll::Ready(None) => {
          // Stream exhausted, complete
          if let Some(observer) = this.observer.take() {
            observer.complete();
          }
          return Poll::Ready(());
        }
        Poll::Pending => {
          // No value available yet, yield control
          return Poll::Pending;
        }
      }
    }
  }
}

impl<St, C, S, Item, Err> CoreObservable<C> for FromStreamResult<St, S>
where
  C: Context,
  C::Inner: Observer<Item, Err>,
  St: Stream<Item = Result<Item, Err>>,
  S: Scheduler<FromStreamResultTask<St, C::Inner>> + Clone,
{
  type Unsub = crate::scheduler::TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let task = FromStreamResultTask { stream: self.stream, observer: Some(observer) };
    self.scheduler.schedule(task, None)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use futures::stream;

  use crate::prelude::*;
  #[rxrust_macro::test(local)]
  async fn test_from_stream_basic() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Local::from_stream(stream::iter(vec![1, 2, 3]))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2, 3]);
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_empty() {
    let next_count = Arc::new(Mutex::new(0));
    let next_count_clone = next_count.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Local::from_stream(stream::iter(Vec::<i32>::new()))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |_| *next_count_clone.lock().unwrap() += 1);

    handle.await;

    assert_eq!(*next_count.lock().unwrap(), 0);
    assert!(*completed.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_stream_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Shared::from_stream(stream::iter(vec![10, 20, 30]))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![10, 20, 30]);
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_unfold() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    let stream =
      stream::unfold(
        1,
        |state| async move { if state <= 3 { Some((state, state + 1)) } else { None } },
      );

    let handle =
      Local::from_stream(stream).subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2, 3]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_stream_channel() {
    use futures::{SinkExt, channel::mpsc};

    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    let (mut sender, receiver) = mpsc::channel::<i32>(8);

    let handle = Shared::from_stream(receiver).subscribe(move |v| {
      result_clone.lock().unwrap().push(v);
    });

    sender.send(10).await.unwrap();
    sender.send(20).await.unwrap();
    sender.send(30).await.unwrap();
    drop(sender);

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![10, 20, 30]);
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_with_map() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    let handle = Local::from_stream(stream::iter(vec![1, 2, 3]))
      .map(|v| v * 2)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![2, 4, 6]);
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_complex_chain() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let handle = Local::from_stream(stream::iter(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]))
      .map(|v| v * 2)
      .filter(|v| *v > 5)
      .take(4)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![6, 8, 10, 12]);
    assert!(*completed.lock().unwrap());
  }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod cancellation_tests {
  use std::sync::{Arc, Mutex};

  use futures::{SinkExt, channel::mpsc, stream};

  use crate::{prelude::*, subscription::Subscription};
  /// 测试 unsubscribe 停止轮询
  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_unsubscribe_stops_polling() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    let (mut sender, receiver) = mpsc::channel::<i32>(8);

    let handle = Shared::from_stream(receiver).subscribe(move |v| {
      result_clone.lock().unwrap().push(v);
    });

    sender.send(1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let handle_clone = handle.clone();
    handle.unsubscribe();

    assert!(handle_clone.is_closed());

    let _ = sender.send(2).await;
    let _ = sender.send(3).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let values = result.lock().unwrap().clone();
    assert_eq!(values, vec![1], "Only values before unsubscribe should be received");
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_cancellation_prevents_completion() {
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let (mut sender, receiver) = mpsc::channel::<i32>(8);

    let handle = Shared::from_stream(receiver)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(|_| {});

    sender.send(1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    handle.unsubscribe();
    drop(sender);

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(!*completed.lock().unwrap(), "Completion should not be called on cancellation");
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_resource_cleanup_on_completion() {
    let resource_active = Arc::new(Mutex::new(true));
    let resource_active_clone = resource_active.clone();

    struct ResourceObserver {
      resource_active: Arc<Mutex<bool>>,
    }

    impl Drop for ResourceObserver {
      fn drop(&mut self) { *self.resource_active.lock().unwrap() = false; }
    }

    let observer = Arc::new(ResourceObserver { resource_active: resource_active_clone });
    let observer_clone = observer.clone();

    let handle = Shared::from_stream(stream::iter(vec![1, 2, 3])).subscribe(move |_v| {
      let _ = &observer_clone;
    });

    drop(observer);
    handle.await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(!*resource_active.lock().unwrap(), "Resource should be cleaned up after completion");
  }
}

#[cfg(test)]
mod pending_tests {
  use std::sync::{Arc, Mutex};

  use futures::stream;

  use crate::prelude::*;

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_pending_does_not_cause_premature_completion() {
    use futures::{SinkExt, channel::mpsc};

    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let (mut sender, receiver) = mpsc::channel::<i32>(8);

    let handle = Shared::from_stream(receiver)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    for _ in 0..3 {
      tokio::time::sleep(Duration::from_millis(20)).await;
      assert!(!*completed.lock().unwrap(), "Stream should NOT complete while pending");
    }

    sender.send(10).await.unwrap();
    sender.send(20).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!*completed.lock().unwrap(), "Stream should NOT complete while sender is open");

    drop(sender);
    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![10, 20]);
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_explicit_pending_control() {
    use std::task::Poll;

    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let mut state = 0;
    let stream = stream::poll_fn(move |_cx| {
      state += 1;
      match state {
        1 => Poll::Ready(Some(1)),
        2 => Poll::Ready(Some(2)),
        3 => Poll::Ready(Some(3)),
        _ => Poll::Ready(None),
      }
    });

    let handle = Local::from_stream(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2, 3]);
    assert!(*completed.lock().unwrap());
  }
}

#[cfg(test)]
mod from_stream_result_tests {
  use std::sync::{Arc, Mutex};

  use futures::stream;

  use crate::prelude::*;
  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_all_ok() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(false));
    let error_clone = error_received.clone();

    let stream = stream::iter(vec![Ok::<_, String>(1), Ok(2), Ok(3)]);

    let handle = Local::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |_| *error_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2, 3]);
    assert!(*completed.lock().unwrap());
    assert!(!*error_received.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_with_error() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    let stream = stream::iter(vec![
      Ok(1),
      Ok(2),
      Err("test error".to_string()),
      Ok(3), // This should not be emitted
    ]);

    let handle = Local::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2]);
    assert!(!*completed.lock().unwrap());
    assert_eq!(*error_received.lock().unwrap(), Some("test error".to_string()));
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_immediate_error() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    let stream = stream::iter(vec![Err::<i32, _>("immediate error".to_string())]);

    let handle = Local::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert!(result.lock().unwrap().is_empty());
    assert!(!*completed.lock().unwrap());
    assert_eq!(*error_received.lock().unwrap(), Some("immediate error".to_string()));
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_empty() {
    let next_count = Arc::new(Mutex::new(0));
    let next_count_clone = next_count.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(false));
    let error_clone = error_received.clone();

    let stream = stream::iter(Vec::<Result<i32, String>>::new());

    let handle = Local::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |_| *error_clone.lock().unwrap() = true)
      .subscribe(move |_| *next_count_clone.lock().unwrap() += 1);

    handle.await;

    assert_eq!(*next_count.lock().unwrap(), 0);
    assert!(*completed.lock().unwrap());
    assert!(!*error_received.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_stream_result_shared_ok() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(false));
    let error_clone = error_received.clone();

    let stream = stream::iter(vec![Ok::<_, String>(10), Ok(20), Ok(30)]);

    let handle = Shared::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |_| *error_clone.lock().unwrap() = true)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![10, 20, 30]);
    assert!(*completed.lock().unwrap());
    assert!(!*error_received.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_stream_result_shared_error() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    let stream = stream::iter(vec![Ok(1), Err("shared error".to_string()), Ok(2)]);

    let handle = Shared::from_stream_result(stream)
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1]);
    assert!(!*completed.lock().unwrap());
    assert_eq!(*error_received.lock().unwrap(), Some("shared error".to_string()));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_from_stream_result_channel() {
    use futures::{SinkExt, channel::mpsc};

    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    let (mut sender, receiver) = mpsc::channel::<Result<i32, String>>(8);

    let handle = Shared::from_stream_result(receiver)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    sender.send(Ok(1)).await.unwrap();
    sender.send(Ok(2)).await.unwrap();
    sender
      .send(Err("channel error".to_string()))
      .await
      .unwrap();
    // These should not be received
    let _ = sender.send(Ok(3)).await;

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2]);
    assert_eq!(*error_received.lock().unwrap(), Some("channel error".to_string()));
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_with_map() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    let stream = stream::iter(vec![Ok::<_, String>(1), Ok(2), Ok(3)]);

    let handle = Local::from_stream_result(stream)
      .map(|v| v * 2)
      .on_error(|_| {})
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![2, 4, 6]);
  }

  #[rxrust_macro::test(local)]
  async fn test_from_stream_result_try_unfold() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();
    let error_received = Arc::new(Mutex::new(None));
    let error_clone = error_received.clone();

    // Create a stream that emits 1, 2, 3 then errors at 4
    let stream = stream::try_unfold(1, |state| async move {
      if state == 4 { Err("invalid value".to_string()) } else { Ok(Some((state, state + 1))) }
    });

    let handle = Local::from_stream_result(stream)
      .on_error(move |e| *error_clone.lock().unwrap() = Some(e))
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    handle.await;

    assert_eq!(*result.lock().unwrap(), vec![1, 2, 3]);
    assert_eq!(*error_received.lock().unwrap(), Some("invalid value".to_string()));
  }
}
