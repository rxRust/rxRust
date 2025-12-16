//! Scan operator implementation
//!
//! This module contains the Scan operator, which applies an accumulator
//! function over the source Observable and returns each intermediate result.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Scan operator: Applies an accumulator function and emits intermediate
/// results
///
/// This operator demonstrates:
/// - Observer wrapping with state (ScanObserver)
/// - Context unpacking with `transform`
/// - Accumulation logic with initial value
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4]).scan(0, |acc, v| acc + v);
/// let mut result = Vec::new();
/// observable.subscribe(|v| {
///   result.push(v);
/// });
/// assert_eq!(result, vec![1, 3, 6, 10]);
/// ```
#[derive(Clone)]
pub struct Scan<S, F, Output> {
  pub source: S,
  pub func: F,
  pub initial_value: Output,
}

impl<S, F, Output> ObservableType for Scan<S, F, Output>
where
  S: ObservableType,
  F: for<'a> FnMut(Output, S::Item<'a>) -> Output,
  Output: Clone,
{
  type Item<'a>
    = Output
  where
    Self: 'a;
  type Err = S::Err;
}

/// ScanObserver wrapper for accumulating values
///
/// This observer wraps another observer and applies an accumulator function
/// to each value, emitting each intermediate accumulation.
pub struct ScanObserver<O, F, Output> {
  observer: O,
  func: F,
  acc: Output,
}

impl<O, F, Item, Output, Err> Observer<Item, Err> for ScanObserver<O, F, Output>
where
  O: Observer<Output, Err>,
  F: FnMut(Output, Item) -> Output,
  Output: Clone,
{
  fn next(&mut self, value: Item) {
    self.acc = (self.func)(self.acc.clone(), value);
    self.observer.next(self.acc.clone());
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C, Output> CoreObservable<C> for Scan<S, F, Output>
where
  C: Context,
  S: CoreObservable<C::With<ScanObserver<C::Inner, F, Output>>>,
  F: for<'a> FnMut(Output, S::Item<'a>) -> Output,
  Output: Clone,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Scan { source, func, initial_value } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| ScanObserver { observer, func, acc: initial_value });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_scan_initial() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 1, 1, 1, 1])
      .scan(100, |acc, v| acc + v)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![101, 102, 103, 104, 105]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_initial_on_empty_observable() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .scan(100, |acc, v| acc + v)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_initial_mixed_types() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(['a', 'b', 'c', 'd', 'e'])
      .scan(100, |acc, _v| acc + 1)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![101, 102, 103, 104, 105]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_multiplication() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3, 4])
      .scan(1, |acc, v| acc * v)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 6, 24]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_string_concatenation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(["hello", " ", "world", "!"])
      .scan("".to_string(), |mut acc, v| {
        acc.push_str(v);
        acc
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(
      *result.borrow(),
      vec![
        "hello".to_string(),
        "hello ".to_string(),
        "hello world".to_string(),
        "hello world!".to_string()
      ]
    );
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_with_observable_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .scan(0, |acc, v| acc + v)
      .map(|v| v * 2)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Original scan would produce: [1, 3, 6, 10, 15]
    // After map(*2): [2, 6, 12, 20, 30]
    assert_eq!(*result.borrow(), vec![2, 6, 12, 20, 30]);
  }
}
