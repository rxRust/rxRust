//! FilterMap operator implementation
//!
//! This module contains the FilterMap operator, which maps each item to an
//! Option<T> and emits only the Some(T) values, effectively combining filtering
//! and mapping.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// FilterMap operator: Maps items to Option and emits only Some values
///
/// This operator maps each item from the source observable to an Option<T>,
/// emitting only the values that are Some(T). It effectively combines filtering
/// and mapping in a single operation.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable =
///   Local::from_iter(["1", "2", "invalid", "3"]).filter_map(|s| s.parse::<i32>().ok());
/// let mut result = Vec::new();
/// observable.subscribe(|v| {
///   result.push(v);
/// });
/// assert_eq!(result, vec![1, 2, 3]);
/// ```
#[derive(Clone)]
pub struct FilterMap<S, F> {
  pub source: S,
  pub func: F,
}

impl<S, F, Out> ObservableType for FilterMap<S, F>
where
  S: ObservableType,
  F: for<'a> FnMut(S::Item<'a>) -> Option<Out>,
{
  type Item<'a>
    = Out
  where
    Self: 'a;
  type Err = S::Err;
}

/// FilterMapObserver wrapper for filtering and mapping
///
/// This observer wraps another observer and applies a function that maps
/// each value to an Option, only passing Some(T) values to the wrapped
/// observer.
pub struct FilterMapObserver<O, F> {
  observer: O,
  func: F,
}

impl<O, F, Item, Out, Err> Observer<Item, Err> for FilterMapObserver<O, F>
where
  O: Observer<Out, Err>,
  F: FnMut(Item) -> Option<Out>,
{
  fn next(&mut self, v: Item) {
    if let Some(mapped) = (self.func)(v) {
      self.observer.next(mapped);
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C, Out> CoreObservable<C> for FilterMap<S, F>
where
  C: Context,
  S: CoreObservable<C::With<FilterMapObserver<C::Inner, F>>>,
  F: for<'a> FnMut(S::Item<'a>) -> Option<Out>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let FilterMap { source, func } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| FilterMapObserver { observer, func });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_filter_map_parse_strings() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec!["1", "2", "invalid", "3", "not_a_number"])
      .filter_map(|s| s.parse::<i32>().ok())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_filter_map_some_none() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3, 4, 5])
      .filter_map(|x| if x % 2 == 0 { Some(x * 2) } else { None })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![4, 8]); // 2*2, 4*2
  }

  #[rxrust_macro::test]
  fn test_filter_map_all_none() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3])
      .filter_map(|_| None)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test]
  fn test_filter_map_all_some() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![1, 2, 3])
      .filter_map(|x| Some(x * 10))
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![10, 20, 30]);
  }

  #[rxrust_macro::test]
  fn test_filter_map_empty_source() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec![] as Vec<i32>)
      .filter_map(|x| Some(x * 2))
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
  }

  #[rxrust_macro::test]
  fn test_filter_map_chaining_with_map() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec!["1", "2", "invalid", "3"])
      .filter_map(|s| s.parse::<i32>().ok())
      .map(|x| x * 10)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![10, 20, 30]);
  }

  #[rxrust_macro::test]
  fn test_filter_map_type_transformation() {
    let result = Rc::new(RefCell::new(Vec::<usize>::new()));
    let result_clone = result.clone();

    Local::from_iter(vec!["hello", "world", "", "rust"])
      .filter_map(|s| if s.len() > 3 { Some(s.len()) } else { None })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![5, 5, 4]); // "hello".len(), "world".len(), "rust".len()
  }
}
