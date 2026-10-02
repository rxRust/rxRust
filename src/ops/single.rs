//! Single operator implementation
//!
//! Emits the only item of a source, or errors if there is none or more than
//! one.

use std::fmt;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Why `single` could not produce exactly one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleError {
  /// The source completed without emitting
  Empty,
  /// The source emitted a second item
  TooMany,
}

impl fmt::Display for SingleError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SingleError::Empty => f.write_str("expected exactly one item, got none"),
      SingleError::TooMany => f.write_str("expected exactly one item, got more"),
    }
  }
}

impl std::error::Error for SingleError {}

/// Single operator: Emit the only item, or error
///
/// Emits the item on completion. Errors with [`SingleError::Empty`] if the
/// source completes without items and with [`SingleError::TooMany`] as soon
/// as a second item arrives, releasing the source.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = None;
/// Local::from_iter(vec![42])
///   .map_err(|_: std::convert::Infallible| SingleError::Empty)
///   .single()
///   .on_error(|_| {})
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(42));
/// ```
#[derive(Clone)]
pub struct Single<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for Single<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that holds the first item and rejects a second
pub struct SingleObserver<O, Item> {
  observer: Option<O>,
  first: Option<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for SingleObserver<O, Item>
where
  O: Observer<Item, Err>,
  Err: From<SingleError>,
{
  fn next(&mut self, value: Item) {
    if self.observer.is_none() {
      return;
    }
    if self.first.is_some() {
      self.first = None;
      if let Some(observer) = self.observer.take() {
        observer.error(SingleError::TooMany.into());
      }
    } else {
      self.first = Some(value);
    }
  }

  fn error(self, e: Err) {
    if let Some(observer) = self.observer {
      observer.error(e);
    }
  }

  fn complete(self) {
    let SingleObserver { observer, first } = self;
    let Some(mut observer) = observer else { return };
    match first {
      Some(value) => {
        observer.next(value);
        observer.complete();
      }
      None => observer.error(SingleError::Empty.into()),
    }
  }

  fn is_closed(&self) -> bool {
    self
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

impl<S, C, Unsub> CoreObservable<C> for Single<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<SingleObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped =
      context.transform(|observer| SingleObserver { observer: Some(observer), first: None });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use super::SingleError;
  use crate::prelude::*;

  fn run(items: Vec<i32>) -> (Vec<i32>, Option<SingleError>, bool) {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(None));
    let completed = Rc::new(RefCell::new(false));
    let (r, e, c) = (result.clone(), error.clone(), completed.clone());
    Local::from_iter(items)
      .map_err(|_: Infallible| SingleError::Empty)
      .single()
      .on_error(move |err| *e.borrow_mut() = Some(err))
      .on_complete(move || *c.borrow_mut() = true)
      .subscribe(move |v| r.borrow_mut().push(v));
    (result.borrow().clone(), *error.borrow(), *completed.borrow())
  }

  #[rxrust_macro::test]
  fn test_single_one_item() {
    assert_eq!(run(vec![7]), (vec![7], None, true));
  }

  #[rxrust_macro::test]
  fn test_single_empty_errors() {
    assert_eq!(run(vec![]), (vec![], Some(SingleError::Empty), false));
  }

  #[rxrust_macro::test]
  fn test_single_too_many_errors_early() {
    let seen = Rc::new(RefCell::new(0));
    let seen_c = seen.clone();
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::from_iter(vec![1, 2, 3, 4])
      .map_err(|_: Infallible| SingleError::Empty)
      .tap(move |_| *seen_c.borrow_mut() += 1)
      .single()
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), Some(SingleError::TooMany));
    // The source was released after the second item
    assert_eq!(*seen.borrow(), 2);
  }

  #[rxrust_macro::test]
  fn test_single_source_error_propagates() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::throw_err(SingleError::TooMany)
      .map(|_| 0)
      .single()
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), Some(SingleError::TooMany));
  }
}
