//! IgnoreElements operator implementation
//!
//! Drops every item and forwards only the terminal notification.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// IgnoreElements operator: Suppresses all items, mirrors error and completion
///
/// Useful when only the outcome of a stream matters, for example waiting for
/// a write to finish.
///
/// # Examples
///
/// ```
/// use std::{cell::Cell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let completed = Rc::new(Cell::new(false));
/// let done = completed.clone();
/// let mut items = Vec::new();
/// Local::from_iter([1, 2, 3])
///   .ignore_elements()
///   .on_complete(move || done.set(true))
///   .subscribe(|v| items.push(v));
/// assert!(items.is_empty());
/// assert!(completed.get());
/// ```
#[doc(alias = "ignoreElements")]
#[derive(Clone)]
pub struct IgnoreElements<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for IgnoreElements<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that discards items
pub struct IgnoreElementsObserver<O> {
  observer: O,
}

impl<O, Item, Err> Observer<Item, Err> for IgnoreElementsObserver<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, _value: Item) {}

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> CoreObservable<C> for IgnoreElements<S>
where
  C: Context,
  S: CoreObservable<C::With<IgnoreElementsObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped = context.transform(|observer| IgnoreElementsObserver { observer });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_ignore_elements_drops_items_and_completes() {
    let items = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let seen = Rc::new(RefCell::new(0));
    let items_c = items.clone();
    let completed_c = completed.clone();
    let seen_c = seen.clone();

    Local::from_iter([1, 2, 3])
      .tap(move |_| *seen_c.borrow_mut() += 1)
      .ignore_elements()
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| items_c.borrow_mut().push(v));

    assert_eq!(*items.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
    // The source still produced every item; only the downstream was shielded
    assert_eq!(*seen.borrow(), 3);
  }

  #[rxrust_macro::test]
  fn test_ignore_elements_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .ignore_elements()
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
