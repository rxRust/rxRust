//! SkipWhile operator implementation
//!
//! This module contains the SkipWhile operator, which skips values from the
//! source observable as long as a predicate returns true, then emits the rest.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// SkipWhile operator: Skips values while a predicate returns true
///
/// This operator skips values from the source observable as long as the
/// predicate function returns true. Once the predicate returns false,
/// it emits that value and all subsequent values.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip_while(|v| *v < 3);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![3, 4, 5]);
/// ```
#[derive(Clone)]
pub struct SkipWhile<S, P> {
  pub source: S,
  pub predicate: P,
}

impl<S: ObservableType, P> ObservableType for SkipWhile<S, P> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// SkipWhileObserver wrapper for skipping values while predicate is true
///
/// This observer wraps another observer and skips values as long as
/// the predicate returns true. Once predicate returns false, it
/// forwards that value and all subsequent values.
pub struct SkipWhileObserver<O, P> {
  observer: O,
  predicate: P,
  done_skipping: bool,
}

impl<O, P, Item, Err> Observer<Item, Err> for SkipWhileObserver<O, P>
where
  O: Observer<Item, Err>,
  P: FnMut(&Item) -> bool,
{
  fn next(&mut self, v: Item) {
    if self.done_skipping {
      self.observer.next(v);
    } else if !(self.predicate)(&v) {
      self.done_skipping = true;
      self.observer.next(v);
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, P, C> CoreObservable<C> for SkipWhile<S, P>
where
  C: Context,
  S: CoreObservable<C::With<SkipWhileObserver<C::Inner, P>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SkipWhile { source, predicate } = self;
    let wrapped =
      context.transform(|observer| SkipWhileObserver { observer, predicate, done_skipping: false });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_skip_while_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .skip_while(|v| *v < 3)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![3, 4, 5]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_skip_while_all_skipped() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3])
      .skip_while(|v| *v < 100)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_skip_while_none_skipped() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([10, 20, 30])
      .skip_while(|v| *v < 5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // First value (10) already fails predicate, so all are emitted
    assert_eq!(*result.borrow(), vec![10, 20, 30]);
  }

  #[rxrust_macro::test]
  fn test_skip_while_base_function() {
    let completed = Rc::new(RefCell::new(false));
    let next_count = Rc::new(RefCell::new(0));
    let completed_clone = completed.clone();
    let next_count_clone = next_count.clone();

    Local::from_iter(0..100)
      .skip_while(|v| *v < 95)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |_| {
        *next_count_clone.borrow_mut() += 1;
      });

    assert_eq!(*next_count.borrow(), 5);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_skip_while_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
      .skip_while(|v| *v < 3)
      .skip_while(|v| *v < 5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // First skip_while emits 3..10, second skip_while skips 3,4 and emits 5..10
    assert_eq!(*result.borrow(), vec![5, 6, 7, 8, 9, 10]);
  }

  #[rxrust_macro::test]
  fn test_skip_while_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .skip_while(|_: &()| true)
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test]
  fn test_skip_while_with_map() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .map(|v| v * 2)
      .skip_while(|v| *v < 6)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // 1*2=2 (skip), 2*2=4 (skip), 3*2=6 (emit), 4*2=8 (emit), 5*2=10 (emit)
    assert_eq!(*result.borrow(), vec![6, 8, 10]);
  }

  #[rxrust_macro::test]
  fn test_skip_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip_while5 = Local::from_iter(0..100).skip_while(|v| *v < 95);
      let f1 = skip_while5.clone();
      let f2 = skip_while5;

      f1.subscribe(|_| nc1 += 1);
      f2.subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }
}
