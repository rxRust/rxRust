//! IsEmpty operator implementation
//!
//! Emits `true` if the source completes without emitting, `false` on the
//! first item.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// IsEmpty operator: Reports whether the source emitted anything
///
/// Emits `false` and completes on the first item, unsubscribing the source.
/// Emits `true` and completes when the source completes without items.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = None;
/// Local::from_iter([1, 2])
///   .is_empty()
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(false));
///
/// let mut result = None;
/// Local::from_iter(std::iter::empty::<i32>())
///   .is_empty()
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(true));
/// ```
#[doc(alias = "isEmpty")]
#[derive(Clone)]
pub struct IsEmpty<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for IsEmpty<S> {
  type Item<'a>
    = bool
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that reports emptiness and short-circuits on the first item
pub struct IsEmptyObserver<O> {
  observer: Option<O>,
}

impl<O, Item, Err> Observer<Item, Err> for IsEmptyObserver<O>
where
  O: Observer<bool, Err>,
{
  fn next(&mut self, _value: Item) {
    if let Some(mut observer) = self.observer.take() {
      observer.next(false);
      observer.complete();
    }
  }

  fn error(self, e: Err) {
    if let Some(observer) = self.observer {
      observer.error(e);
    }
  }

  fn complete(self) {
    if let Some(mut observer) = self.observer {
      observer.next(true);
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

impl<S, C> CoreObservable<C> for IsEmpty<S>
where
  C: Context,
  S: CoreObservable<C::With<IsEmptyObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped = context.transform(|observer| IsEmptyObserver { observer: Some(observer) });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_is_empty_false_short_circuits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(0));
    let result_c = result.clone();
    let seen_c = seen.clone();

    Local::from_iter([1, 2, 3])
      .tap(move |_| *seen_c.borrow_mut() += 1)
      .is_empty()
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![false]);
    assert_eq!(*seen.borrow(), 1);
  }

  #[rxrust_macro::test]
  fn test_is_empty_true_on_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .is_empty()
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![true]);
  }

  #[rxrust_macro::test]
  fn test_is_empty_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .is_empty()
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
