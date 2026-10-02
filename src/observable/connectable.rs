//! ConnectableObservable implementation for multicasting.
//!
//! `ConnectableObservable` acts as a bridge between a source `Observable` and a
//! `Subject`. It allows multicasting a single source execution to multiple
//! subscribers.
//!
//! # Key Concepts
//!
//! - **Not an Observable**: It does not emit upon subscription.
//! - **Connect**: `connect()` must be called to start the source execution.
//! - **Fork**: `fork()` creates the actual `Observable` instances for
//!   subscribers.
//!
//! # Example
//!
//! ```rust
//! use rxrust::prelude::*;
//!
//! let source = Local::of(1).merge(Local::of(2));
//! let subject = Local::subject();
//! let connectable = source.multicast(subject.into_inner());
//!
//! // Create multiple observers sharing the same source connection
//! let obs1 = connectable.fork();
//! let obs2 = connectable.fork();
//!
//! obs1.subscribe(|v| println!("Observer 1: {}", v));
//! obs2.subscribe(|v| println!("Observer 2: {}", v));
//!
//! // Start execution
//! connectable.connect();
//! ```

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  ops::ref_count::RefCount,
};

/// ConnectableObservable: A special Observable that requires manual connection
///
/// **⚠️ Important**: This is NOT a regular Observable. It is an auxiliary
/// object that bridges a source Observable with a subject to enable
/// multicasting.
///
/// **How it differs from a regular Observable**:
/// - **Regular Observable**: Starts emitting immediately upon subscription
/// - **ConnectableObservable**: Requires explicit `connect()` call to start
///   emitting
/// - **Subscription**: Use `fork()` to create Observable instances, then
///   subscribe normally
///
/// It holds a source and a subject of any kind (`Subject`, `BehaviorSubject`,
/// `ReplaySubject`, `AsyncSubject`). Subscribers listen to the subject, and
/// `connect()` subscribes the subject to the source.
#[derive(Clone)]
pub struct ConnectableObservable<S, Sub> {
  pub(crate) source: S,
  pub(crate) subject: Sub,
}

// Observer implementation: Delegates to the internal subject
impl<Item, Err, S, Sub> Observer<Item, Err> for ConnectableObservable<S, Sub>
where
  Sub: Observer<Item, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) { self.subject.next(value); }

  #[inline]
  fn error(self, err: Err) { self.subject.error(err); }

  #[inline]
  fn complete(self) { self.subject.complete(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subject.is_closed() }
}

impl<S, Sub> ObservableType for ConnectableObservable<S, Sub>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<C, S, Sub> CoreObservable<C> for ConnectableObservable<S, Sub>
where
  S: ObservableType,
  Sub: CoreObservable<C>,
{
  type Unsub = <Sub as CoreObservable<C>>::Unsub;

  fn subscribe(self, observer: C) -> Self::Unsub { self.subject.subscribe(observer) }
}

impl<S, Sub: Clone> ConnectableObservable<S, Sub> {
  /// Creates a new handle on the underlying subject for subscribers.
  pub fn fork(&self) -> Sub { self.subject.clone() }

  /// Subscribes the subject to the source, starting emissions.
  pub fn connect<C: Context>(self) -> S::Unsub
  where
    S: CoreObservable<C::With<Sub>>,
  {
    self.source.subscribe(C::lift(self.subject))
  }
}

/// Unified trait for operations on `ConnectableObservable`.
///
/// Provides `fork`, `connect`, and `ref_count` methods integrated with
/// `Context`.
pub trait Connectable<S, Sub: Clone>: Context<Inner = ConnectableObservable<S, Sub>>
where
  S: CoreObservable<Self::With<Sub>>,
{
  /// Connects the source to the subject, starting emissions.
  ///
  /// Returns a subscription handle to disconnect the source.
  fn connect(self) -> S::Unsub { self.into_inner().connect::<Self>() }

  /// Creates a new `Observable` subscribed to the underlying subject.
  ///
  /// All forks share the same source connection.
  fn fork(&self) -> Self::With<Sub> { self.wrap(self.inner().fork()) }

  /// Returns an Observable that automatically connects when the first
  /// observer subscribes and disconnects when the last one unsubscribes.
  #[allow(clippy::type_complexity)]
  fn ref_count(self) -> Self::With<RefCount<S, Sub, Self::RcMut<Option<S::Unsub>>>> {
    let connectable = self.into_inner();
    let connection = Self::RcMut::from(None);
    Self::lift(RefCount { connectable, connection })
  }
}

impl<C, S, Sub> Connectable<S, Sub> for C
where
  C: Context<Inner = ConnectableObservable<S, Sub>>,
  Sub: Clone,
  S: CoreObservable<C::With<Sub>>,
{
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;
  use crate::prelude::*;

  fn create_value_capture<T>() -> (Rc<RefCell<Vec<T>>>, impl FnMut(T) + Clone)
  where
    T: Clone,
  {
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_clone = values.clone();
    let capture = move |value: T| {
      values_clone.borrow_mut().push(value);
    };
    (values, capture)
  }

  #[rxrust_macro::test]
  fn test_connectable_basic() {
    let source = Local::of(42);
    let subject_context = Local::subject();
    let connectable = source.multicast(subject_context.into_inner());

    let (captured, observer) = create_value_capture();

    connectable.fork().subscribe(observer);
    connectable.connect();

    assert_eq!(*captured.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_connectable_multiple_observers() {
    let source = Local::of(42);
    let subject_context = Local::subject();
    let connectable = source.multicast(subject_context.into_inner());

    let (vals1, obs1) = create_value_capture();
    let (vals2, obs2) = create_value_capture();

    connectable.fork().subscribe(obs1);
    connectable.fork().subscribe(obs2);

    connectable.connect();

    assert_eq!(*vals1.borrow(), vec![42]);
    assert_eq!(*vals2.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_connectable_multiple_emissions() {
    let source = Local::of(1)
      .merge(Local::of(2))
      .merge(Local::of(3));
    let subject_context = Local::subject();
    let connectable = source.multicast(subject_context.into_inner());

    let (vals, obs) = create_value_capture();

    connectable.fork().subscribe(obs);
    connectable.connect();

    assert_eq!(*vals.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_connectable_error() {
    let source = Local::of(42); // Using simple source as error propagation is standard
    let subject_context = Local::subject();
    let connectable = source.multicast(subject_context.into_inner());

    let (vals, obs) = create_value_capture();

    connectable.fork().subscribe(obs);
    connectable.connect();

    assert_eq!(*vals.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_fork_independence() {
    let source = Local::of(42);
    let connectable = source.multicast(Local::subject().into_inner());

    let obs1 = connectable.fork();
    let obs2 = connectable.fork();

    let (vals1, sub1) = create_value_capture();
    let (vals2, sub2) = create_value_capture();

    obs1.subscribe(sub1);
    obs2.subscribe(sub2);

    connectable.connect();

    assert_eq!(*vals1.borrow(), vec![42]);
    assert_eq!(*vals2.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_publish_alias() {
    let source = Local::of(42);
    let connectable = source.publish();

    let (vals, obs) = create_value_capture();

    connectable.fork().subscribe(obs);
    connectable.connect();

    assert_eq!(*vals.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_publish_last_emits_only_final_value() {
    let (results, capture) = create_value_capture();
    let connectable = Local::from_iter(vec![1, 2, 3]).publish_last();
    connectable.fork().subscribe(capture);
    connectable.connect();
    assert_eq!(*results.borrow(), vec![3]);
  }

  #[rxrust_macro::test]
  fn test_publish_behavior_seeds_subscribers() {
    let (results, capture) = create_value_capture();
    let connectable = Local::from_iter(vec![1, 2]).publish_behavior(0);
    connectable.fork().subscribe(capture);
    connectable.connect();
    assert_eq!(*results.borrow(), vec![0, 1, 2]);
  }
}
