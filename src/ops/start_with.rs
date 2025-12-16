//! StartWith operator implementation
//!
//! Emits specified items before beginning to emit items from the source
//! Observable.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// StartWith operator: Emits specified values before the source values
///
/// This operator prepends provided items to the source observable,
/// emitting them immediately before subscribing to the source.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([3, 4, 5])
///   .start_with(vec![1, 2])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3, 4, 5]);
/// ```
#[derive(Clone)]
pub struct StartWith<S, Item> {
  pub source: S,
  pub values: Vec<Item>,
}

impl<S, Item> ObservableType for StartWith<S, Item>
where
  S: ObservableType,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, C, Item> CoreObservable<C> for StartWith<S, Item>
where
  C: Context,
  C::Inner: Observer<Item, S::Err>,
  S: CoreObservable<C>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, mut context: C) -> Self::Unsub {
    let StartWith { source, values } = self;
    let observer = context.inner_mut();

    // Emit all initial values first
    for value in values {
      if observer.is_closed() {
        break;
      }
      observer.next(value);
    }

    source.subscribe(context)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_start_with_integers() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .start_with(vec![-1, 0])
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![-1, 0, 1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_start_with_strings() {
    let result = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();

    Local::from_iter([" World!", " Goodbye", " World!"])
      .start_with(vec!["Hello"])
      .subscribe(move |v| {
        result_clone.borrow_mut().push_str(v);
      });

    assert_eq!(*result.borrow(), "Hello World! Goodbye World!");
  }

  #[rxrust_macro::test]
  fn test_start_with_empty_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .start_with(vec![])
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_start_with_single_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([2, 3])
      .start_with(vec![1])
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_start_with_chained() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([5])
      .start_with(vec![3, 4])
      .start_with(vec![1, 2])
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4, 5]);
  }
}
