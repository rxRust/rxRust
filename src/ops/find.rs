//! Find operators implementation
//!
//! `find` emits the first item matching a predicate; `find_index` emits its
//! zero-based position.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  ops::{filter::Filter, take::Take},
};

/// Emits the first item satisfying the predicate, then completes. Completes
/// empty when nothing matches. Composed from `filter` and `take`.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 4, 6, 8])
///   .find(|v| v % 2 == 0)
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![4]);
/// ```
pub type Find<S, F> = Take<Filter<S, F>>;

/// FindIndex operator: Emits the zero-based index of the first match
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 4, 6, 8])
///   .find_index(|v| v % 2 == 0)
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1]);
/// ```
#[doc(alias = "findIndex")]
#[derive(Clone)]
pub struct FindIndex<S, F> {
  pub source: S,
  pub predicate: F,
}

impl<S, F> ObservableType for FindIndex<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = usize
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that counts positions until the predicate matches
pub struct FindIndexObserver<O, F> {
  observer: Option<O>,
  predicate: F,
  index: usize,
}

impl<O, F, Item, Err> Observer<Item, Err> for FindIndexObserver<O, F>
where
  O: Observer<usize, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, v: Item) {
    if self.observer.is_none() {
      return;
    }
    if (self.predicate)(&v) {
      if let Some(mut observer) = self.observer.take() {
        observer.next(self.index);
        observer.complete();
      }
    } else {
      self.index += 1;
    }
  }

  fn error(self, e: Err) {
    if let Some(observer) = self.observer {
      observer.error(e);
    }
  }

  fn complete(self) {
    if let Some(observer) = self.observer {
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

impl<S, F, C> CoreObservable<C> for FindIndex<S, F>
where
  C: Context,
  S: CoreObservable<C::With<FindIndexObserver<C::Inner, F>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let FindIndex { source, predicate } = self;
    let wrapped = context.transform(|observer| FindIndexObserver {
      observer: Some(observer),
      predicate,
      index: 0,
    });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_find_first_match_and_stop() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(0));
    let result_c = result.clone();
    let seen_c = seen.clone();

    Local::from_iter([1, 3, 4, 6])
      .tap(move |_| *seen_c.borrow_mut() += 1)
      .find(|v| v % 2 == 0)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![4]);
    assert_eq!(*seen.borrow(), 3);
  }

  #[rxrust_macro::test]
  fn test_find_no_match_completes_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter([1, 3, 5])
      .find(|v| v % 2 == 0)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_find_index_first_match() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1, 3, 4, 6])
      .find_index(|v| v % 2 == 0)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![2]);
  }

  #[rxrust_macro::test]
  fn test_find_index_no_match_completes_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter([1, 3, 5])
      .find_index(|v| v % 2 == 0)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<usize>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_find_index_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .find_index(|v| *v == 1)
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
