//! EndWith operator implementation
//!
//! Emits specified items after the source completes.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// EndWith operator: Appends values after the source completes
///
/// The values are emitted in order when the source completes, then the
/// stream completes. They are not emitted if the source errors.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 2])
///   .end_with(vec![3, 4])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3, 4]);
/// ```
#[doc(alias = "endWith")]
#[derive(Clone)]
pub struct EndWith<S, Item> {
  pub source: S,
  pub values: Vec<Item>,
}

impl<S, Item> ObservableType for EndWith<S, Item>
where
  S: ObservableType,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that flushes the trailing values on completion
pub struct EndWithObserver<O, Item> {
  observer: O,
  values: Vec<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for EndWithObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) {
    let EndWithObserver { mut observer, values } = self;
    for value in values {
      if observer.is_closed() {
        break;
      }
      observer.next(value);
    }
    observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C, Item> CoreObservable<C> for EndWith<S, Item>
where
  C: Context,
  S: CoreObservable<C::With<EndWithObserver<C::Inner, Item>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let EndWith { source, values } = self;
    let wrapped = context.transform(|observer| EndWithObserver { observer, values });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_end_with_appends_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1, 2])
      .end_with(vec![3, 4])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn test_end_with_on_empty_source() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .end_with(vec![9])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![9]);
  }

  #[rxrust_macro::test]
  fn test_end_with_not_emitted_on_error() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_c = result.clone();
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .end_with(vec![1])
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert_eq!(*error.borrow(), "boom");
  }

  #[rxrust_macro::test]
  fn test_end_with_respects_take() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1])
      .end_with(vec![2, 3, 4])
      .take(2)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
