//! ThrowIfEmpty operator implementation
//!
//! Errors instead of completing when the source emits nothing.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// ThrowIfEmpty operator: Turns an empty completion into an error
///
/// If the source completes without emitting, `error_fn` is called and its
/// result is emitted as the error. Otherwise the operator is transparent.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let error = Rc::new(RefCell::new(None));
/// let sink = error.clone();
/// Local::from_iter(std::iter::empty::<i32>())
///   .map_err(|_: std::convert::Infallible| String::new())
///   .throw_if_empty(|| "empty".to_string())
///   .on_error(move |e| *sink.borrow_mut() = Some(e))
///   .subscribe(|_| {});
/// assert_eq!(error.borrow().as_deref(), Some("empty"));
/// ```
#[doc(alias = "throwIfEmpty")]
#[derive(Clone)]
pub struct ThrowIfEmpty<S, F> {
  pub source: S,
  pub error_fn: F,
}

impl<S, F> ObservableType for ThrowIfEmpty<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that tracks whether anything was emitted
pub struct ThrowIfEmptyObserver<O, F> {
  observer: O,
  error_fn: F,
  is_empty: bool,
}

impl<O, F, Item, Err> Observer<Item, Err> for ThrowIfEmptyObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnOnce() -> Err,
{
  fn next(&mut self, value: Item) {
    self.is_empty = false;
    self.observer.next(value);
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) {
    let ThrowIfEmptyObserver { observer, error_fn, is_empty } = self;
    if is_empty {
      observer.error(error_fn());
    } else {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C> CoreObservable<C> for ThrowIfEmpty<S, F>
where
  C: Context,
  S: CoreObservable<C::With<ThrowIfEmptyObserver<C::Inner, F>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let ThrowIfEmpty { source, error_fn } = self;
    let wrapped =
      context.transform(|observer| ThrowIfEmptyObserver { observer, error_fn, is_empty: true });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_throw_if_empty_errors_on_empty() {
    let error = Rc::new(RefCell::new(None));
    let completed = Rc::new(RefCell::new(false));
    let error_c = error.clone();
    let completed_c = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .map_err(|_: Infallible| String::new())
      .throw_if_empty(|| "empty".to_string())
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert_eq!(error.borrow().as_deref(), Some("empty"));
    assert!(!*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_throw_if_empty_transparent_when_items() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter([1, 2])
      .map_err(|_: Infallible| String::new())
      .throw_if_empty(|| "empty".to_string())
      .on_error(|_| unreachable!())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_throw_if_empty_forwards_source_error() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .throw_if_empty(|| "empty".to_string())
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
