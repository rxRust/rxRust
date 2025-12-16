//! Pairwise operator implementation
//!
//! This module contains the Pairwise operator, which groups consecutive
//! emissions from the source Observable into tuples `(previous, current)`.

use crate::{
  context::Context, observable::CoreObservable, observer::Observer, prelude::ObservableType,
  subscription::Subscription,
};

/// Pairwise operator: Groups consecutive emissions into tuples
///
/// This operator groups consecutive emissions from the source Observable into
/// tuples `(previous, current)`. On the first emission, it stores the value.
/// On subsequent emissions, it emits `(stored, current)` and updates the stored
/// value.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4]).pairwise();
/// let mut result = Vec::new();
/// observable.subscribe(|pair| result.push(pair));
/// assert_eq!(result, vec![(1, 2), (2, 3), (3, 4)]);
/// ```
#[derive(Clone)]
pub struct Pairwise<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for Pairwise<S> {
  type Item<'a>
    = (S::Item<'a>, S::Item<'a>)
  where
    Self: 'a;
  type Err = S::Err;
}

/// PairwiseObserver wrapper for grouping consecutive values into tuples
///
/// This observer wraps another observer and groups consecutive values into
/// tuples. The first value is stored, and each subsequent value is paired with
/// the previous one.
pub struct PairwiseObserver<O, Item> {
  observer: O,
  prev: Option<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for PairwiseObserver<O, Item>
where
  O: Observer<(Item, Item), Err>,
  Item: Clone,
{
  fn next(&mut self, value: Item) {
    if let Some(prev_value) = self.prev.take() {
      // Emit the pair (previous, current)
      self.observer.next((prev_value, value.clone()));
    }
    // Store the current value for the next emission
    self.prev = Some(value);
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C, Unsub> CoreObservable<C> for Pairwise<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<PairwiseObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| PairwiseObserver { observer, prev: None });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_pairwise_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .pairwise()
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(*result.borrow(), vec![(1, 2), (2, 3), (3, 4), (4, 5)]);
  }

  #[rxrust_macro::test]
  fn test_pairwise_single_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([42])
      .pairwise()
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    // No pairs should be emitted with only one value
    assert_eq!(*result.borrow(), vec![]);
  }

  #[rxrust_macro::test]
  fn test_pairwise_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([] as [i32; 0])
      .pairwise()
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    // No pairs should be emitted with empty source
    assert_eq!(*result.borrow(), vec![]);
  }

  #[rxrust_macro::test]
  fn test_pairwise_two_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([10, 20])
      .pairwise()
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(*result.borrow(), vec![(10, 20)]);
  }

  #[rxrust_macro::test]
  fn test_pairwise_with_strings() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(["hello".to_string(), "world".to_string(), "test".to_string()])
      .pairwise()
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(
      *result.borrow(),
      vec![("hello".to_string(), "world".to_string()), ("world".to_string(), "test".to_string())]
    );
  }

  #[rxrust_macro::test]
  fn test_pairwise_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .pairwise()
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(*result.borrow(), vec![]);
    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test]
  fn test_pairwise_completion() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3])
      .pairwise()
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(*result.borrow(), vec![(1, 2), (2, 3)]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_pairwise_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4])
      .pairwise()
      .map(|(a, b)| a + b)
      .subscribe(move |sum| {
        result_clone.borrow_mut().push(sum);
      });

    // (1+2), (2+3), (3+4) = 3, 5, 7
    assert_eq!(*result.borrow(), vec![3, 5, 7]);
  }

  #[rxrust_macro::test]
  fn test_pairwise_take_after_pairwise() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .pairwise()
      .take(2)
      .subscribe(move |pair| {
        result_clone.borrow_mut().push(pair);
      });

    assert_eq!(*result.borrow(), vec![(1, 2), (2, 3)]);
  }
}
