//! IntoFuture operator implementation
//!
//! This module provides functionality to convert an Observable
//! into a Future that resolves with the single value emitted by the observable.
//!
//! ## Behavior
//!
//! - **Single value**: If the observable emits exactly one value, the future
//!   resolves with `Ok(Ok(value))`
//! - **Error from observable**: If the observable emits an error, the future
//!   resolves with `Ok(Err(error))`
//! - **Empty observable**: If the observable completes without emitting any
//!   values, the future resolves with `Err(IntoFutureError::Empty)`
//! - **Multiple values**: If the observable emits more than one value, the
//!   future resolves with `Err(IntoFutureError::MultipleValues)`
//!
//! ## Examples
//!
//! ```rust,no_run
//! use rxrust::prelude::*;
//!
//! async fn test_into_future() {
//!   // Single value
//!   let fut = Local::of(42).into_future();
//!   let value = fut.await.unwrap().ok().unwrap();
//!   assert_eq!(value, 42);
//!
//!   // Empty observable
//!   let fut = Local::from_iter(std::iter::empty::<i32>()).into_future();
//!   assert!(matches!(fut.await, Err(IntoFutureError::Empty)));
//!
//!   // Multiple values
//!   let fut = Local::from_iter([1, 2, 3]).into_future();
//!   assert!(matches!(fut.await, Err(IntoFutureError::MultipleValues)));
//! }
//! ```

use std::{
  cell::RefCell,
  fmt::Display,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context as TaskContext, Poll, Waker},
};

use crate::{
  context::Context,
  observable::{CoreObservable, Observable, ObservableType},
  observer::Observer,
};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can prevent an observable future from resolving correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntoFutureError {
  /// The observable had no values.
  Empty,

  /// The observable emitted more than one value.
  MultipleValues,
}

impl Display for IntoFutureError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      IntoFutureError::Empty => write!(f, "the observable has no values"),
      IntoFutureError::MultipleValues => {
        write!(f, "the observable emitted more than one value")
      }
    }
  }
}

impl std::error::Error for IntoFutureError {}

/// The result type for `into_future()`.
///
/// - `Ok(Ok(value))` - Observable emitted exactly one value
/// - `Ok(Err(error))` - Observable emitted an error
/// - `Err(IntoFutureError::Empty)` - Observable completed without emitting
/// - `Err(IntoFutureError::MultipleValues)` - Observable emitted more than one
///   value
pub type IntoFutureResult<T, E> = Result<Result<T, E>, IntoFutureError>;

// ============================================================================
// Internal State
// ============================================================================

/// Internal state for the IntoFutureObserver
pub(crate) enum State<Item, Err> {
  /// No value received yet
  Empty,
  /// Exactly one value received
  HasValue(Item),
  /// More than one value received
  MultipleValues,
  /// An error was received
  Error(Err),
}

/// Shared state between Future and Observer
pub(crate) struct SharedState<Item, Err> {
  pub(crate) state: State<Item, Err>,
  pub(crate) waker: Option<Waker>,
  pub(crate) completed: bool,
}

// ============================================================================
// ObservableFuture
// ============================================================================

/// A future that resolves with the value emitted by an observable.
///
/// This future supports both synchronous and asynchronous observables.
/// It uses a shared state with a waker to properly await async observables.
pub struct ObservableFuture<Item, Err> {
  shared: Rc<RefCell<SharedState<Item, Err>>>,
}

impl<Item, Err> Future for ObservableFuture<Item, Err> {
  type Output = IntoFutureResult<Item, Err>;

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let mut shared = self.shared.borrow_mut();
    if shared.completed {
      let result = match std::mem::replace(&mut shared.state, State::Empty) {
        State::Empty => Err(IntoFutureError::Empty),
        State::HasValue(v) => Ok(Ok(v)),
        State::MultipleValues => Err(IntoFutureError::MultipleValues),
        State::Error(e) => Ok(Err(e)),
      };
      Poll::Ready(result)
    } else {
      shared.waker = Some(cx.waker().clone());
      Poll::Pending
    }
  }
}

// ============================================================================
// IntoFutureObserver
// ============================================================================

/// Observer that collects the last value for `into_future()`.
///
/// This observer stores the result in a shared state so it can be
/// retrieved after subscription completes. It supports both synchronous
/// and asynchronous observables by using a waker to notify the future.
pub struct IntoFutureObserver<Item, Err> {
  shared: Rc<RefCell<SharedState<Item, Err>>>,
}

impl<Item, Err> IntoFutureObserver<Item, Err> {
  pub(crate) fn new(shared: Rc<RefCell<SharedState<Item, Err>>>) -> Self { Self { shared } }

  /// Wake the future if a waker is registered
  fn wake(&self) {
    if let Some(waker) = self.shared.borrow_mut().waker.take() {
      waker.wake();
    }
  }
}

impl<Item, Err> Observer<Item, Err> for IntoFutureObserver<Item, Err> {
  fn next(&mut self, value: Item) {
    let mut shared = self.shared.borrow_mut();
    match &shared.state {
      State::Empty => shared.state = State::HasValue(value),
      State::HasValue(_) => {
        shared.state = State::MultipleValues;
        shared.completed = true;
        drop(shared);
        self.wake();
      }
      // Already in terminal state, ignore
      State::MultipleValues | State::Error(_) => {}
    }
  }

  fn error(self, err: Err) {
    {
      let mut shared = self.shared.borrow_mut();
      shared.state = State::Error(err);
      shared.completed = true;
    }
    self.wake();
  }

  fn complete(self) {
    self.shared.borrow_mut().completed = true;
    self.wake();
  }

  fn is_closed(&self) -> bool {
    // Stop receiving if we already have multiple values or an error
    let shared = self.shared.borrow();
    shared.completed || matches!(shared.state, State::MultipleValues | State::Error(_))
  }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Creates a future from an observable.
///
/// This function subscribes to the observable and returns a future that
/// resolves when the observable completes. It supports both synchronous
/// and asynchronous observables.
pub fn observable_into_future<T, E, F>(subscribe_fn: F) -> ObservableFuture<T, E>
where
  F: FnOnce(IntoFutureObserver<T, E>),
{
  let shared =
    Rc::new(RefCell::new(SharedState { state: State::Empty, waker: None, completed: false }));
  let observer = IntoFutureObserver::new(shared.clone());
  subscribe_fn(observer);

  ObservableFuture { shared }
}

// ============================================================================
// SupportsIntoFuture Trait (internal conversion capability)
// ============================================================================

/// Internal capability trait used by `Observable::into_future()` to avoid
/// exposing `IntoFutureObserver` in public bounds.
///
/// This trait is intentionally `#[doc(hidden)]`. Users should call
/// `Observable::into_future()` instead of depending on this directly.
#[doc(hidden)]
pub trait SupportsIntoFuture<'a, C>: ObservableType + Sized
where
  C: Context<Inner = Self> + Observable + 'a,
  Self: 'a,
{
  /// Convert a context-wrapped observable into an [`ObservableFuture`].
  fn into_future(ctx: C) -> ObservableFuture<C::Item<'a>, C::Err>;
}

impl<'a, C, T> SupportsIntoFuture<'a, C> for T
where
  C: Context<Inner = T> + Observable + 'a,
  T: ObservableType + 'a,
  // Use fully qualified syntax to break the cycle in trait bound computation
  T: CoreObservable<C::With<IntoFutureObserver<C::Item<'a>, C::Err>>>,
{
  fn into_future(ctx: C) -> ObservableFuture<C::Item<'a>, C::Err> {
    observable_into_future(|observer| {
      let (core, wrapped) = ctx.swap(observer);
      // NOTE: we currently drop the subscription handle, matching the old behavior.
      core.subscribe(wrapped);
    })
  }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use futures::task::noop_waker;

  use super::*;
  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_into_future_single_value() {
    let fut = Local::of(42).into_future();
    let value = fut.await;
    assert_eq!(value, Ok(Ok(42)));
  }

  #[rxrust_macro::test(local)]
  async fn test_into_future_empty_observable() {
    let fut = Local::from_iter(std::iter::empty::<i32>()).into_future();
    let value = fut.await;
    assert_eq!(value, Err(IntoFutureError::Empty));
  }

  #[rxrust_macro::test(local)]
  async fn test_into_future_multiple_values() {
    let fut = Local::from_iter([1, 2, 3]).into_future();
    let value = fut.await;
    assert_eq!(value, Err(IntoFutureError::MultipleValues));
  }

  #[rxrust_macro::test(local)]
  async fn test_into_future_with_map() {
    let fut = Local::of(4)
      .map(|x| format!("Number {x}"))
      .into_future();
    let value = fut.await.unwrap().ok().unwrap();
    assert_eq!(value, "Number 4");
  }

  #[rxrust_macro::test(local)]
  async fn test_into_future_error() {
    let fut = Local::throw_err("test error".to_string()).into_future();
    let value = fut.await;
    assert_eq!(value, Ok(Err("test error".to_string())));
  }

  #[rxrust_macro::test]
  fn test_into_future_with_delay() {
    TestScheduler::init();

    // Create a delayed observable
    let mut fut = TestCtx::of(42)
      .delay(Duration::from_millis(100))
      .into_future();

    // First poll should return Pending (value not yet emitted)
    let waker = noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    let poll_result = Pin::new(&mut fut).poll(&mut cx);
    assert!(poll_result.is_pending());

    // Advance time to trigger the delayed emission
    TestScheduler::advance_by(Duration::from_millis(100));

    // Now poll should return Ready with the value
    let poll_result = Pin::new(&mut fut).poll(&mut cx);
    assert_eq!(poll_result, Poll::Ready(Ok(Ok(42))));
  }

  #[rxrust_macro::test]
  fn test_into_future_sync_observable_completes_immediately() {
    // Synchronous observables should have completed = true after subscribe
    let shared = Rc::new(RefCell::new(SharedState::<i32, ()> {
      state: State::Empty,
      waker: None,
      completed: false,
    }));
    let observer = IntoFutureObserver::new(shared.clone());

    // Simulate a sync observable
    let mut observer_mut = observer;
    observer_mut.next(42);

    // For sync case, we need to manually complete
    {
      let mut s = shared.borrow_mut();
      s.completed = true;
    }

    // Create future and poll
    let mut fut = ObservableFuture { shared };
    let waker = noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    let poll_result = Pin::new(&mut fut).poll(&mut cx);

    // Should be ready immediately for synchronous observables
    assert_eq!(
      poll_result,
      Poll::Ready(Ok(Ok(42))),
      "Synchronous observable should complete immediately"
    );
  }
}
