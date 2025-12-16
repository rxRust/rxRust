//! Tap operator implementation
//!
//! This module contains the Tap operator, which performs side effects for
//! each emission without modifying the emitted values.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Tap operator: Performs a side effect for each emission
///
/// The `tap` operator allows you to perform side effects for each value
/// emitted by the source observable, without modifying the values themselves.
/// It's useful for debugging, logging, or triggering external actions.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let sum = Rc::new(RefCell::new(0));
/// let sum_clone = sum.clone();
/// Local::from_iter([1, 2, 3])
///   .tap(move |v| *sum_clone.borrow_mut() += *v)
///   .subscribe(|_| {});
/// assert_eq!(*sum.borrow(), 6);
/// ```
#[derive(Clone)]
pub struct Tap<S, F> {
  pub source: S,
  pub func: F,
}

/// TapObserver wrapper for performing side effects
///
/// This observer wraps another observer and calls a side-effect function
/// for each value before passing it to the wrapped observer.
pub struct TapObserver<O, F> {
  observer: O,
  func: F,
}

impl<O, F, Item, Err> Observer<Item, Err> for TapObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item),
{
  fn next(&mut self, v: Item) {
    (self.func)(&v);
    self.observer.next(v);
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F> ObservableType for Tap<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, C> CoreObservable<C> for Tap<S, F>
where
  C: Context,
  S: CoreObservable<C::With<TapObserver<C::Inner, F>>>,
  F: for<'a> FnMut(&S::Item<'a>),
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Tap { source, func } = self;
    let wrapped = context.transform(|observer| TapObserver { observer, func });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_tap_side_effect() {
    let tap_values = Rc::new(RefCell::new(Vec::new()));
    let tap_clone = tap_values.clone();
    let sub_values = Rc::new(RefCell::new(Vec::new()));
    let sub_clone = sub_values.clone();

    Local::from_iter([1, 2, 3])
      .tap(move |v| tap_clone.borrow_mut().push(*v))
      .subscribe(move |v| sub_clone.borrow_mut().push(v));

    assert_eq!(*tap_values.borrow(), vec![1, 2, 3]);
    assert_eq!(*sub_values.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_tap_with_map() {
    let tap_values = Rc::new(RefCell::new(Vec::new()));
    let tap_clone = tap_values.clone();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .tap(move |v| tap_clone.borrow_mut().push(*v))
      .map(|v| v * 2)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    // tap sees original values
    assert_eq!(*tap_values.borrow(), vec![1, 2, 3]);
    // subscribe sees mapped values
    assert_eq!(*result.borrow(), vec![2, 4, 6]);
  }

  #[rxrust_macro::test(local)]
  async fn test_tap_does_not_modify_values() {
    let result = Rc::new(RefCell::new(vec![]));
    let result_clone = result.clone();

    Local::from_iter([10, 20, 30])
      .tap(|_| {
        // Side effect only, no modification
      })
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![10, 20, 30]);
  }

  #[rxrust_macro::test(local)]
  async fn test_tap_error_propagation() {
    let tapped = Rc::new(RefCell::new(false));
    let tapped_clone = tapped.clone();
    let error_received = Rc::new(RefCell::new(false));
    let error_clone = error_received.clone();

    Local::throw_err("test error".to_string())
      .tap(move |_: &()| *tapped_clone.borrow_mut() = true)
      .on_error(move |_| *error_clone.borrow_mut() = true)
      .subscribe(|_| {});

    // tap should not be called on error
    assert!(!*tapped.borrow());
    // error should propagate
    assert!(*error_received.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_tap_chaining() {
    let tap1_values = Rc::new(RefCell::new(Vec::new()));
    let tap1_clone = tap1_values.clone();
    let tap2_values = Rc::new(RefCell::new(Vec::new()));
    let tap2_clone = tap2_values.clone();

    Local::from_iter([1, 2, 3])
      .tap(move |v| tap1_clone.borrow_mut().push(*v))
      .map(|v| v * 10)
      .tap(move |v| tap2_clone.borrow_mut().push(*v))
      .subscribe(|_| {});

    assert_eq!(*tap1_values.borrow(), vec![1, 2, 3]);
    assert_eq!(*tap2_values.borrow(), vec![10, 20, 30]);
  }
}
