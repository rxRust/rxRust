//! Trivial Observable implementations
//!
//! This module provides simple Observable implementations that emit limited or
//! no values. These are useful for testing, edge cases, and specific reactive
//! patterns.
//!
//! The types in this module are:
//! - [`Empty`]: Completes immediately without emitting any values
//! - [`Never`]: Never emits any values and never completes
//! - [`ThrowErr`]: Immediately emits an error without any values

use std::convert::Infallible;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// An Observable that completes immediately without emitting any values.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let completed = Rc::new(RefCell::new(false));
/// let next_called = Rc::new(RefCell::new(false));
///
/// let completed_c = completed.clone();
/// let next_called_c = next_called.clone();
///
/// Local::empty()
///   .on_complete(move || *completed_c.borrow_mut() = true)
///   .subscribe(move |_| *next_called_c.borrow_mut() = true);
///
/// assert!(*completed.borrow());
/// assert!(!*next_called.borrow());
/// ```
///
/// # Use Cases
///
/// - Testing completion behavior without values
/// - Creating base cases in conditional observable chains
/// - Representing empty collections as observables
/// - Signaling the end of a stream without data
#[derive(Default, Clone, Copy)]
pub struct Empty;

impl ObservableType for Empty {
  type Item<'a>
    = ()
  where
    Self: 'a;
  type Err = Infallible;
}

impl<C> CoreObservable<C> for Empty
where
  C: Context,
  C::Inner: Observer<(), Infallible>,
{
  type Unsub = ();

  fn subscribe(self, context: C) -> Self::Unsub { context.into_inner().complete(); }
}

/// An Observable that never emits any values and never completes.
///
/// This represents an infinite stream that never produces output. It's useful
/// for scenarios where you need a source that never terminates naturally.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let completed = Rc::new(RefCell::new(false));
/// let next_called = Rc::new(RefCell::new(false));
///
/// let completed_c = completed.clone();
/// let next_called_c = next_called.clone();
///
/// // Note: In a real scenario, you'd typically need to handle this with
/// // timeout or take operators since it never completes
/// let subscription = Local::never()
///   .on_complete(move || *completed_c.borrow_mut() = true)
///   .subscribe(move |_| *next_called_c.borrow_mut() = true);
///
/// // Neither completion nor next should be called
/// assert!(!*completed.borrow());
/// assert!(!*next_called.borrow());
/// drop(subscription);
/// ```
///
/// # Use Cases
///
/// - Testing timeout and cancellation behavior
/// - Creating infinite streams that are terminated manually
/// - Implementing retry mechanisms that continue until explicitly stopped
/// - Testing operators that should work with non-terminating streams
///
/// Note
///
/// Since this observable never completes, you should typically use operators
/// like `take()`, `timeout()`, or manually unsubscribe to avoid memory leaks.
#[derive(Default, Clone, Copy)]
pub struct Never;

impl ObservableType for Never {
  type Item<'a>
    = ()
  where
    Self: 'a;
  type Err = Infallible;
}

impl<C> CoreObservable<C> for Never
where
  C: Context,
  C::Inner: Observer<(), Infallible>,
{
  type Unsub = ();

  fn subscribe(self, _context: C) -> Self::Unsub {}
}

/// An Observable that immediately emits an error without emitting any values.
///
/// This is useful for testing error handling, creating error-based fallbacks,
/// or representing failure conditions as observables.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let error_received = Rc::new(RefCell::new(None));
/// let completed = Rc::new(RefCell::new(false));
/// let next_called = Rc::new(RefCell::new(false));
///
/// let error_received_c = error_received.clone();
/// let completed_c = completed.clone();
/// let next_called_c = next_called.clone();
///
/// Local::throw_err("Connection failed".to_string())
///   .on_complete(move || *completed_c.borrow_mut() = true)
///   .on_error(move |e| *error_received_c.borrow_mut() = Some(e))
///   .subscribe(move |_: ()| *next_called_c.borrow_mut() = true);
///
/// assert!(!*next_called.borrow());
/// assert!(!*completed.borrow());
/// assert_eq!(*error_received.borrow(), Some("Connection failed".to_string()));
/// ```
///
/// # Use Cases
///
/// - Testing error handling in observable chains
/// - Creating fallback observables that represent failure conditions
/// - Implementing circuit breaker patterns
/// - Representing network or resource failures as observables
/// - Testing operators' behavior with error streams
///
/// # Type Parameters
///
/// * `E` - The error type that will be emitted
#[derive(Clone)]
pub struct ThrowErr<E> {
  pub error: E,
}

impl<E> ObservableType for ThrowErr<E> {
  type Item<'a>
    = ()
  where
    Self: 'a;
  type Err = E;
}

impl<C, E> CoreObservable<C> for ThrowErr<E>
where
  C: Context,
  C::Inner: Observer<(), E>,
{
  type Unsub = ();

  fn subscribe(self, context: C) -> Self::Unsub { context.into_inner().error(self.error); }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_empty() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();
    let hits = Rc::new(RefCell::new(0));
    let hits_clone = hits.clone();

    Local::empty()
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |_| *hits_clone.borrow_mut() += 1);

    assert_eq!(*hits.borrow(), 0);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_never() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();
    let hits = Rc::new(RefCell::new(0));
    let hits_clone = hits.clone();

    Local::never()
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |_| *hits_clone.borrow_mut() += 1);

    assert_eq!(*hits.borrow(), 0);
    assert!(!*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_throw_err() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();
    let hits = Rc::new(RefCell::new(0));
    let hits_clone = hits.clone();
    let error = Rc::new(RefCell::new(String::new()));
    let error_clone = error.clone();

    Local::throw_err("error".to_string())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .on_error(move |e| *error_clone.borrow_mut() = e)
      .subscribe(move |_| *hits_clone.borrow_mut() += 1);

    assert_eq!(*hits.borrow(), 0);
    assert!(!*completed.borrow());
    assert_eq!(*error.borrow(), "error");
  }
}
