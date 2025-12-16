//! DefaultIfEmpty operator implementation
//!
//! Emits a default value if the source Observable completes without emitting
//! any items.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// DefaultIfEmpty operator: Emits a default value if the source is empty
///
/// This operator emits a default value if the source Observable completes
/// without emitting any items. If the source emits any items, the default
/// value is not emitted.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::empty()
///   .map_to(0)
///   .default_if_empty(42)
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![42]);
/// ```
#[derive(Clone)]
pub struct DefaultIfEmpty<S, Item> {
  pub(crate) source: S,
  pub(crate) default_value: Item,
}

impl<S, Item> DefaultIfEmpty<S, Item> {
  pub(crate) fn new(source: S, default_value: Item) -> Self { Self { source, default_value } }
}

impl<S, Item> ObservableType for DefaultIfEmpty<S, Item>
where
  S: ObservableType,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, Item, C> CoreObservable<C> for DefaultIfEmpty<S, Item>
where
  C: Context,
  S: CoreObservable<C::With<DefaultIfEmptyObserver<C::Inner, Item>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DefaultIfEmpty { source, default_value } = self;

    let wrapped = context.transform(|observer| DefaultIfEmptyObserver {
      observer,
      is_empty: true,
      default_value,
    });

    source.subscribe(wrapped)
  }
}

/// DefaultIfEmptyObserver: wrapper for tracking emissions
pub struct DefaultIfEmptyObserver<O, Item> {
  observer: O,
  is_empty: bool,
  default_value: Item,
}

impl<O, Item, Err> Observer<Item, Err> for DefaultIfEmptyObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.is_empty = false;
    self.observer.next(value);
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) {
    let DefaultIfEmptyObserver { mut observer, is_empty, default_value } = self;
    if is_empty {
      observer.next(default_value);
    }
    observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn default_if_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::of(10)
      .default_if_empty(5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![10]);
  }

  #[rxrust_macro::test(local)]
  async fn default_if_empty_when_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .default_if_empty(5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![5]);
  }

  #[rxrust_macro::test(local)]
  async fn test_non_clone_item() {
    struct NonClone(i32);
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<NonClone>())
      .default_if_empty(NonClone(42))
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v.0);
      });

    assert_eq!(*result.borrow(), vec![42]);
  }
}
