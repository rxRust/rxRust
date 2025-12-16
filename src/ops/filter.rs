//! Filter operator implementation
//!
//! This module contains the Filter operator, which emits only those items from
//! the source Observable that pass a predicate test.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Filter operator: Emits only items that pass a predicate test
///
/// This operator filters items from the source observable, only emitting those
/// for which the predicate returns true.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).filter(|v| v % 2 == 0);
/// let mut result = Vec::new();
/// observable.subscribe(|v| {
///   result.push(v);
/// });
/// assert_eq!(result, vec![2, 4]);
/// ```
#[derive(Clone)]
pub struct Filter<S, F> {
  pub source: S,
  pub filter: F,
}

impl<S: ObservableType, F> ObservableType for Filter<S, F> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// FilterObserver wrapper for predicate filtering
///
/// This observer wraps another observer and applies a predicate function
/// to each value, only passing it to the wrapped observer if the predicate
/// returns true.
pub struct FilterObserver<O, F> {
  observer: O,
  filter: F,
}

impl<O, F, Item, Err> Observer<Item, Err> for FilterObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, v: Item) {
    if (self.filter)(&v) {
      self.observer.next(v);
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C> CoreObservable<C> for Filter<S, F>
where
  C: Context,
  S: CoreObservable<C::With<FilterObserver<C::Inner, F>>>,
  F: for<'a> FnMut(&S::Item<'a>) -> bool,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Filter { source, filter } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| FilterObserver { observer, filter });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_filter_even_numbers() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3, 4, 5, 6])
      .filter(|v| v % 2 == 0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![2, 4, 6]);
  }

  #[rxrust_macro::test(local)]
  async fn test_filter_string_length() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec!["a", "hello", "world", "hi", "test"])
      .filter(|s| s.len() > 3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v.to_string());
      });

    assert_eq!(*result.borrow(), vec!["hello", "world", "test"]);
  }

  #[rxrust_macro::test(local)]
  async fn test_filter_all_pass() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3])
      .filter(|_| true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_filter_none_pass() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3])
      .filter(|_| false)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test(local)]
  async fn test_filter_empty_source() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![] as Vec<i32>)
      .filter(|v| v % 2 == 0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test(local)]
  async fn test_filter_chaining_with_map() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
      .filter(|x| x % 2 == 0)
      .map(|x| x * 2)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0, 4, 8, 12, 16]);
  }
}
