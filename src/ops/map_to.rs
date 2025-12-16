//! MapTo operator implementation
//!
//! This module contains the MapTo operator, which maps every emission from the
//! source observable to a constant value. It requires the constant value to be
//! `Clone` since it is emitted multiple times.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// MapTo operator: Maps every emission to a constant value
///
/// This operator demonstrates:
/// - Observer wrapping (MapToObserver)
/// - Context unpacking with `transform`
/// - Constant value mapping with Clone requirement
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::of(42).map_to(0);
/// let mut result = None;
/// observable.subscribe(|v| {
///   result = Some(v);
/// });
/// assert_eq!(result, Some(0));
/// ```
#[derive(Clone)]
pub struct MapTo<S, B> {
  pub source: S,
  pub value: B,
}

impl<S, B> ObservableType for MapTo<S, B>
where
  S: ObservableType,
  B: Clone,
{
  type Item<'a>
    = B
  where
    Self: 'a;
  type Err = S::Err;
}

/// MapToObserver wrapper for mapping values to a constant
///
/// This observer wraps another observer and emits a constant value
/// for every item received from the source.
pub struct MapToObserver<O, B> {
  observer: O,
  value: B,
}

impl<O, B, Item, Err> Observer<Item, Err> for MapToObserver<O, B>
where
  O: Observer<B, Err>,
  B: Clone,
{
  fn next(&mut self, _item: Item) { self.observer.next(self.value.clone()); }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, B, C> CoreObservable<C> for MapTo<S, B>
where
  C: Context,
  S: CoreObservable<C::With<MapToObserver<C::Inner, B>>>,
  B: Clone,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let MapTo { source, value } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| MapToObserver { observer, value });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_map_to_primitive_type() {
    let result = Rc::new(RefCell::new(0));
    let result_clone = result.clone();

    Local::of(42).map_to(5).subscribe(move |v| {
      *result_clone.borrow_mut() += v;
    });

    assert_eq!(*result.borrow(), 5);
  }

  #[rxrust_macro::test]
  fn test_map_to_reference_lifetime() {
    let result = Rc::new(RefCell::new(0));
    let result_clone = result.clone();

    Local::of(100).map_to(5).subscribe(move |v| {
      *result_clone.borrow_mut() += v;
    });

    assert_eq!(*result.borrow(), 5);
  }

  #[rxrust_macro::test]
  fn test_map_to_multiple_values() {
    let result = Rc::new(RefCell::new(0));
    let result_clone = result.clone();

    // Test with multiple separate emissions using from_iter
    Local::from_iter(['a', 'b', 'c'])
      .map_to(1)
      .subscribe(move |v| {
        *result_clone.borrow_mut() += v;
      });

    assert_eq!(*result.borrow(), 3);
  }

  #[rxrust_macro::test]
  fn test_map_to_string_constant() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .map_to("constant".to_string())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(
      *result.borrow(),
      vec!["constant".to_string(), "constant".to_string(), "constant".to_string()]
    );
  }

  #[rxrust_macro::test]
  fn test_map_to_chaining() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::of(42)
      .map(|x| x / 2)
      .map_to("final".to_string())
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*result.borrow(), Some("final".to_string()));
  }

  #[rxrust_macro::test]
  fn test_map_to_empty_observable() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(Vec::<i32>::new())
      .map_to(42)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test]
  fn test_map_to_with_error() {
    let error_received = Rc::new(RefCell::new(false));
    let error_received_clone = error_received.clone();

    // Create an observable that emits an error and subscribe to it
    Local::throw_err("test error")
      .map_to(99)
      .on_error(move |_err| *error_received_clone.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*error_received.borrow());
  }
}
