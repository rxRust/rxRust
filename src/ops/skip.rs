//! Skip operator implementation
//!
//! This module contains the Skip operator, which ignores the first `count`
//! values emitted by the source Observable, then emits the rest.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Skip operator: Ignores the first `count` values from the source observable
///
/// This operator skips the first `count` values emitted by the source
/// observable, then emits all subsequent values. If the source completes
/// before emitting `count` values, `skip` will complete without emitting
/// any values.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip(2);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![3, 4, 5]);
/// ```
#[derive(Clone)]
pub struct Skip<S> {
  pub source: S,
  pub count: usize,
}

impl<S: ObservableType> ObservableType for Skip<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// SkipObserver wrapper for skipping the first `count` values
///
/// This observer wraps another observer and ignores the first `count`
/// values received. After `count` values have been skipped, it forwards
/// all subsequent values to the inner observer.
pub struct SkipObserver<O> {
  observer: O,
  remaining: usize,
}

impl<O, Item, Err> Observer<Item, Err> for SkipObserver<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, v: Item) {
    if self.remaining > 0 {
      self.remaining -= 1;
    } else {
      self.observer.next(v);
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> CoreObservable<C> for Skip<S>
where
  C: Context,
  S: CoreObservable<C::With<SkipObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Skip { source, count } = self;
    let wrapped = context.transform(|observer| SkipObserver { observer, remaining: count });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_skip_ignores_first_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .skip(2)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![3, 4, 5]);
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_base_function() {
    let completed = Rc::new(RefCell::new(false));
    let next_count = Rc::new(RefCell::new(0));
    let completed_clone = completed.clone();
    let next_count_clone = next_count.clone();

    Local::from_iter(0..100)
      .skip(5)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |_| {
        *next_count_clone.borrow_mut() += 1;
      });

    assert_eq!(*next_count.borrow(), 95);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_more_than_source() {
    let completed = Rc::new(RefCell::new(false));
    let next_count = Rc::new(RefCell::new(0));
    let completed_clone = completed.clone();
    let next_count_clone = next_count.clone();

    Local::from_iter(0..100)
      .skip(101)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |_| {
        *next_count_clone.borrow_mut() += 1;
      });

    assert_eq!(*next_count.borrow(), 0);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_with_zero_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .skip(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(0..100)
      .skip(5)
      .skip(5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(result.borrow().len(), 90);
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .skip(5)
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![]);
    assert_eq!(*error.borrow(), "test error");
  }
}
