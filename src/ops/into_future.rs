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
  fmt::Display,
  future::Future,
  pin::Pin,
  task::{Context as TaskContext, Poll, Waker},
};

use crate::{
  context::{Context, RcDerefMut},
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
#[doc(hidden)]
pub struct SharedState<Item, Err> {
  pub(crate) state: State<Item, Err>,
  pub(crate) waker: Option<Waker>,
  pub(crate) completed: bool,
}

type IntoFutureHandle<C, Item, Err> = <C as Context>::RcMut<SharedState<Item, Err>>;

/// The concrete future returned by [`Observable::into_future()`] for a given
/// observable context.
pub type ObservableFutureOf<'a, C> =
  ObservableFuture<IntoFutureHandle<C, <C as Observable>::Item<'a>, <C as Observable>::Err>>;

// ============================================================================
// ObservableFuture
// ============================================================================

/// A future that resolves with the value emitted by an observable.
///
/// This future supports both synchronous and asynchronous observables.
/// It uses a shared state with a waker to properly await async observables.
pub struct ObservableFuture<R> {
  shared: R,
}

impl<R, Item, Err> Future for ObservableFuture<R>
where
  R: RcDerefMut<Target = SharedState<Item, Err>> + Clone,
{
  type Output = IntoFutureResult<Item, Err>;

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
    let mut shared = self.shared.rc_deref_mut();
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
pub struct IntoFutureObserver<R> {
  shared: R,
}

impl<R> Clone for IntoFutureObserver<R>
where
  R: Clone,
{
  fn clone(&self) -> Self { Self { shared: self.shared.clone() } }
}

impl<R, Item, Err> IntoFutureObserver<R>
where
  R: RcDerefMut<Target = SharedState<Item, Err>> + Clone,
{
  pub(crate) fn new(shared: R) -> Self { Self { shared } }

  /// Wake the future if a waker is registered
  fn wake(&self) {
    if let Some(waker) = self.shared.rc_deref_mut().waker.take() {
      waker.wake();
    }
  }
}

impl<R, Item, Err> Observer<Item, Err> for IntoFutureObserver<R>
where
  R: RcDerefMut<Target = SharedState<Item, Err>> + Clone,
{
  fn next(&mut self, value: Item) {
    let mut shared = self.shared.rc_deref_mut();
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
      let mut shared = self.shared.rc_deref_mut();
      shared.state = State::Error(err);
      shared.completed = true;
    }
    self.wake();
  }

  fn complete(self) {
    self.shared.rc_deref_mut().completed = true;
    self.wake();
  }

  fn is_closed(&self) -> bool {
    // Stop receiving if we already have multiple values or an error
    let shared = self.shared.rc_deref();
    shared.completed || matches!(shared.state, State::MultipleValues | State::Error(_))
  }
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
  fn into_future(ctx: C) -> ObservableFutureOf<'a, C>;
}

impl<'a, C, T> SupportsIntoFuture<'a, C> for T
where
  C: Context<Inner = T> + Observable + 'a,
  T: ObservableType + 'a,
  // Use fully qualified syntax to break the cycle in trait bound computation
  T: CoreObservable<C::With<IntoFutureObserver<IntoFutureHandle<C, C::Item<'a>, C::Err>>>>,
{
  fn into_future(ctx: C) -> ObservableFutureOf<'a, C> {
    let shared: IntoFutureHandle<C, C::Item<'a>, C::Err> =
      SharedState { state: State::Empty, waker: None, completed: false }.into();
    let observer = IntoFutureObserver::new(shared.clone());
    let future = ObservableFuture { shared };
    let (core, wrapped) = ctx.swap(observer);
    // NOTE: we currently drop the subscription handle, matching the old behavior.
    core.subscribe(wrapped);
    future
  }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use futures::task::noop_waker;

  use super::*;
  use crate::{
    prelude::*,
    rc::{MutRc, RcDerefMut},
  };

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
    let shared =
      MutRc::from(SharedState::<i32, ()> { state: State::Empty, waker: None, completed: false });
    let observer = IntoFutureObserver::new(shared.clone());

    // Simulate a sync observable
    let mut observer_mut = observer;
    observer_mut.next(42);

    // For sync case, we need to manually complete
    {
      let mut s = shared.rc_deref_mut();
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

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_into_future_shared_with_observe_on() {
    let fut = Shared::of(42)
      .observe_on(SharedScheduler)
      .into_future();

    assert_eq!(fut.await, Ok(Ok(42)));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_into_future_shared_behavior_subject_issue_276() {
    let mut subject = Shared::behavior_subject::<bool, ()>(false);
    let fut = subject
      .clone()
      .observe_on(SharedScheduler)
      .filter(|v| *v)
      .first()
      .into_future();

    subject.next(true);

    assert_eq!(fut.await, Ok(Ok(true)));
  }
}
