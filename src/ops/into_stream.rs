//! IntoStream Operator
//!
//! This module provides the functionality to convert an `Observable` into a
//! `futures_core::stream::Stream`.
//!
//! The conversion allows seamless integration of RxRust observables with the
//! async/await ecosystem, enabling users to consume observable emissions using
//! standard stream patterns like `while let` loops.
//!
//! # Example
//!
//! ```rust
//! use futures::StreamExt;
//! use rxrust::prelude::*;
//!
//! # async fn example() {
//! let mut stream = Local::of(1).into_stream();
//!
//! if let Some(Ok(value)) = stream.next().await {
//!   println!("Received: {}", value);
//! }
//! # }
//! ```

use std::{
  collections::VecDeque,
  pin::Pin,
  task::{Context as AsyncContext, Poll, Waker},
};

use futures_core::stream::Stream;

use crate::{
  observable::{CoreObservable, Observable, ObservableType},
  observer::Observer,
  rc::RcDerefMut,
  subscription::Subscription,
};

/// Internal state shared between the Observable subscription and the Stream
/// consumer.
///
/// This struct holds the data buffer and synchronization primitives.
#[doc(hidden)]
pub struct IntoStreamState<T, E> {
  /// Buffer for items and errors waiting to be polled.
  queue: VecDeque<Result<T, E>>,
  /// The waker for the async task waiting on the stream.
  waker: Option<Waker>,
  /// Flag indicating if the upstream observable has completed or errored.
  is_closed: bool,
}

impl<T, E> Default for IntoStreamState<T, E> {
  fn default() -> Self { Self { queue: VecDeque::new(), waker: None, is_closed: false } }
}

/// A `Stream` that yields values emitted by an `Observable`.
///
/// This struct is created via the
/// [`into_stream`](crate::observable::Observable::into_stream) method.
/// It implements `futures_core::stream::Stream`, yielding items of type
/// `Result<T, E>`.
///
/// - `Ok(T)`: Emitted for each `next` value from the observable.
/// - `Err(E)`: Emitted when the observable signals an `error`.
/// - `None`: Emitted (end of stream) when the observable `completes` or after
///   emitting an error.
pub struct IntoStream<R, U: Subscription> {
  state: R,
  unsub: Option<U>,
}

impl<R, U: Subscription> IntoStream<R, U> {
  /// Constructs a new `IntoStream`.
  ///
  /// This method subscribes to the provided observable and sets up the shared
  /// state for streaming.
  pub fn new<'a, O>(observable: O) -> Self
  where
    O: Observable + 'a,
    R: RcDerefMut<Target = IntoStreamState<O::Item<'a>, O::Err>>
      + From<IntoStreamState<O::Item<'a>, O::Err>>,
    O::Inner: CoreObservable<O::With<IntoStreamObserver<R>>, Unsub = U>,
    U: Subscription,
  {
    let state = R::from(IntoStreamState::default());
    let observer = IntoStreamObserver { state: state.clone() };

    let (core, wrapped) = observable.swap(observer);
    let unsub = core.subscribe(wrapped);

    IntoStream { state, unsub: Some(unsub) }
  }
}

impl<T, E, R, U: Subscription> Stream for IntoStream<R, U>
where
  R: RcDerefMut<Target = IntoStreamState<T, E>>,
{
  type Item = Result<T, E>;

  fn poll_next(self: Pin<&mut Self>, cx: &mut AsyncContext<'_>) -> Poll<Option<Self::Item>> {
    // Safety: we don't move any fields out of `self` here.
    let this = unsafe { self.get_unchecked_mut() };
    let mut state = this.state.rc_deref_mut();

    if let Some(item) = state.queue.pop_front() {
      return Poll::Ready(Some(item));
    }

    if state.is_closed {
      return Poll::Ready(None);
    }

    // Register waker if queue is empty and not closed.
    // We clone the waker to notify the specific task waiting on this stream.
    state.waker = Some(cx.waker().clone());
    Poll::Pending
  }
}

impl<R, U: Subscription> Drop for IntoStream<R, U> {
  fn drop(&mut self) {
    if let Some(unsub) = self.unsub.take() {
      unsub.unsubscribe();
    }
  }
}

/// An `Observer` that forwards emissions into a `State`.
///
/// This observer is used internally by `IntoStream` to capture events
/// from the source observable.
#[doc(hidden)]
pub struct IntoStreamObserver<R> {
  state: R,
}

impl<R, Item, Err> Observer<Item, Err> for IntoStreamObserver<R>
where
  R: RcDerefMut<Target = IntoStreamState<Item, Err>>,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    state.queue.push_back(Ok(value));
    if let Some(waker) = state.waker.take() {
      waker.wake();
    }
  }

  fn error(self, err: Err) {
    let mut state = self.state.rc_deref_mut();
    state.queue.push_back(Err(err));
    state.is_closed = true;
    if let Some(waker) = state.waker.take() {
      waker.wake();
    }
  }

  fn complete(self) {
    let mut state = self.state.rc_deref_mut();
    state.is_closed = true;
    if let Some(waker) = state.waker.take() {
      waker.wake();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref_mut().is_closed }
}

// ============================================================================
// SupportsIntoStream Trait (internal conversion capability)
// ============================================================================

/// Internal capability trait used by `Observable::into_stream()` to avoid
/// exposing `IntoStreamObserver` / `State` in public bounds and return types.
///
/// This trait is intentionally `#[doc(hidden)]`. Users should call
/// `Observable::into_stream()` instead of depending on this directly.
#[doc(hidden)]
pub trait SupportsIntoStream<'a, C>: Sized + ObservableType
where
  C: Observable<Inner = Self> + 'a,
  Self: 'a,
{
  /// The concrete stream type returned for this `(Self, C, 'a)` combination.
  ///
  /// This is a GAT so `Observable::into_stream()` can return an opaque
  /// projection without leaking internal state types.
  type Stream: Stream<Item = Result<C::Item<'a>, C::Err>>;

  /// Convert a context-wrapped observable into a
  /// `futures_core::stream::Stream`.
  fn into_stream(ctx: C) -> Self::Stream;
}

impl<'a, C, T, Unsub> SupportsIntoStream<'a, C> for T
where
  C: Observable<Inner = T> + 'a,
  T: 'a,
  // Ensures `IntoStream::new(ctx)` is valid and hides the observer/state wiring.
  T: CoreObservable<
      C::With<IntoStreamObserver<C::RcMut<IntoStreamState<C::Item<'a>, C::Err>>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Stream = IntoStream<C::RcMut<IntoStreamState<C::Item<'a>, C::Err>>, Unsub>;
  fn into_stream(ctx: C) -> Self::Stream { IntoStream::new(ctx) }
}

#[cfg(test)]
mod tests {
  use futures::StreamExt;

  use super::*;
  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn into_stream_receive_all_values_test() {
    let mut stream = Local::from_iter(vec![1, 2, 3]).into_stream();

    let mut values: Vec<i32> = vec![];
    while let Some(Ok(x)) = stream.next().await {
      values.push(x);
    }

    assert_eq!(vec![1, 2, 3], values);
  }

  #[rxrust_macro::test(local)]
  async fn into_stream_simple_test() {
    let mut stream = Local::of(123).into_stream();
    let item = stream.next().await;
    assert_eq!(item, Some(Ok(123)));
  }

  #[rxrust_macro::test(local)]
  async fn into_stream_empty_observable_test() {
    let mut values: Vec<i32> = vec![];
    let mut stream = Local::from_iter(std::iter::empty::<i32>()).into_stream();

    while let Some(Ok(x)) = stream.next().await {
      values.push(x);
    }

    assert!(values.is_empty());
  }

  #[rxrust_macro::test(local)]
  async fn into_stream_error_test() {
    let mut stream = Local::throw_err("error").into_stream();

    let item = stream.next().await;
    assert_eq!(item, Some(Err("error")));
    let item = stream.next().await;
    assert_eq!(item, None);
  }
}
