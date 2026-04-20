//! ScanMap operator implementation
//!
//! This module contains the ScanMap operator, which applies a function over
//! the source Observable that both mutates an accumulated value and produces
//! a new value.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// ScanMap operator: Transforms each item while accumulating a value
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4]).scan_map(0, |acc, v| {
///   *acc += v;
///   *acc + 10
/// });
/// let mut result = Vec::new();
/// observable.subscribe(|v| {
///   result.push(v);
/// });
/// assert_eq!(result, vec![11, 13, 16, 20]);
/// ```
#[derive(Clone)]
pub struct ScanMap<S, F, Acc> {
  pub source: S,
  pub func: F,
  pub initial_value: Acc,
}

impl<S, F, Acc, Output> ObservableType for ScanMap<S, F, Acc>
where
  S: ObservableType,
  F: for<'a> FnMut(&mut Acc, S::Item<'a>) -> Output,
{
  type Item<'a>
    = Output
  where
    Self: 'a;
  type Err = S::Err;
}

/// ScanMapObserver wrapper for accumulating values
///
/// This observer wraps another observer and applies a function to each value
/// that transforms it and mutates an accumulated value, emitting each
/// transformation.
pub struct ScanMapObserver<O, F, Acc> {
  observer: O,
  func: F,
  acc: Acc,
}

impl<O, F, Item, Acc, Output, Err> Observer<Item, Err> for ScanMapObserver<O, F, Acc>
where
  O: Observer<Output, Err>,
  F: FnMut(&mut Acc, Item) -> Output,
{
  fn next(&mut self, value: Item) {
    let output = (self.func)(&mut self.acc, value);
    self.observer.next(output);
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C, Acc, Output> CoreObservable<C> for ScanMap<S, F, Acc>
where
  C: Context,
  S: CoreObservable<C::With<ScanMapObserver<C::Inner, F, Acc>>>,
  F: for<'a> FnMut(&mut Acc, S::Item<'a>) -> Output,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let ScanMap { source, func, initial_value } = self;
    // Use transform to preserve scheduler automatically
    let wrapped =
      context.transform(|observer| ScanMapObserver { observer, func, acc: initial_value });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_scan_map_initial() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 1, 1, 1, 1])
      .scan_map(100, |acc, v| {
        *acc += v;
        *acc + 100
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![201, 202, 203, 204, 205]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_map_initial_on_empty_observable() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .scan_map(100, |acc, v| {
        *acc += v;
        *acc + 100
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_map_initial_mixed_types() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(['a', 'b', 'c', 'd', 'e'])
      .scan_map(100, |acc, _v| {
        *acc += 1;
        *acc
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![101, 102, 103, 104, 105]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_map_character_concatenation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(['a', ' ', 'b', '!'])
      .scan_map("".to_string(), |acc, v| {
        acc.push(v);
        acc.len()
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test(local)]
  async fn test_scan_map_with_observable_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .scan_map(0, |acc, v| {
        *acc += v;
        *acc + 1
      })
      .filter(|v| v % 2 == 0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Original scan_map would produce: [2, 4, 7, 11, 16]
    // After filter:
    assert_eq!(*result.borrow(), vec![2, 4, 16]);
  }
}
