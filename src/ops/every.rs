//! Every operator implementation
//!
//! Emits `true` if every item satisfies a predicate, or `false` as soon as
//! one item does not.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Every operator: Checks whether every item satisfies a predicate
///
/// Emits `false` and completes as soon as one item fails the predicate,
/// unsubscribing from the source. Emits `true` when the source completes and
/// every item passed. An empty source emits `true`.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = None;
/// Local::from_iter([2, 4, 6])
///   .every(|v| v % 2 == 0)
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(true));
///
/// let mut result = None;
/// Local::from_iter([2, 3, 6])
///   .every(|v| v % 2 == 0)
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(false));
/// ```
#[doc(alias = "all")]
#[derive(Clone)]
pub struct Every<S, F> {
  pub source: S,
  pub predicate: F,
}

impl<S, F> ObservableType for Every<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = bool
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that evaluates the predicate and short-circuits on the first
/// failure
pub struct EveryObserver<O, F> {
  observer: Option<O>,
  predicate: F,
}

impl<O, F, Item, Err> Observer<Item, Err> for EveryObserver<O, F>
where
  O: Observer<bool, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, v: Item) {
    if self.observer.is_none() {
      return;
    }
    if !(self.predicate)(&v)
      && let Some(mut observer) = self.observer.take()
    {
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

impl<S, F, C> CoreObservable<C> for Every<S, F>
where
  C: Context,
  S: CoreObservable<C::With<EveryObserver<C::Inner, F>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Every { source, predicate } = self;
    let wrapped =
      context.transform(|observer| EveryObserver { observer: Some(observer), predicate });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_every_all_pass() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([2, 4, 6])
      .every(|v| v % 2 == 0)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![true]);
  }

  #[rxrust_macro::test]
  fn test_every_short_circuits_on_first_failure() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(0));
    let result_clone = result.clone();
    let seen_clone = seen.clone();

    Local::from_iter([2, 3, 4, 5])
      .tap(move |_| *seen_clone.borrow_mut() += 1)
      .every(|v| v % 2 == 0)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![false]);
    // 2 passes, 3 fails; 4 and 5 are never pulled from the source
    assert_eq!(*seen.borrow(), 2);
  }

  #[rxrust_macro::test]
  fn test_every_empty_is_true() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .every(|_| false)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![true]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_every_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .every(|v| *v == 0)
      .on_error(move |e| *error_clone.borrow_mut() = e)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<bool>::new());
    assert_eq!(*error.borrow(), "boom");
  }

  #[rxrust_macro::test]
  fn test_every_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    Shared::from_iter([1, 2, 3])
      .every(|v| *v > 0)
      .subscribe(move |v| result_clone.lock().unwrap().push(v));

    assert_eq!(*result.lock().unwrap(), vec![true]);
  }
}
