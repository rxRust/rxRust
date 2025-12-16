//! Last operator implementation
//!
//! This module contains the Last operator, which emits only the last value
//! emitted by the source Observable when the source completes.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Last operator: Emits only the last value from the source observable
///
/// This operator keeps track of the last value emitted by the source
/// observable. When the source completes, it emits that last value and then
/// completes. If the source emits no values, it completes without emitting
/// anything.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).last();
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![5]);
/// ```
#[derive(Clone)]
pub struct Last<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for Last<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// LastObserver wrapper for emitting the last value
///
/// This observer wraps another observer and tracks the last value seen.
/// When the source completes, it emits the last value (if any).
pub struct LastObserver<O, Item> {
  observer: O,
  last: Option<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for LastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.last = Some(value); }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(mut self) {
    if let Some(value) = self.last.take() {
      self.observer.next(value);
    }
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C, Unsub> CoreObservable<C> for Last<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<LastObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Last { source } = self;
    let wrapped = context.transform(|observer| LastObserver { observer, last: None });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_last_emits_last_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .last()
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![5]);
  }

  #[rxrust_macro::test]
  fn test_last_with_single_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::of(42).last().subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_last_with_no_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .last()
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should not emit any values but should complete
    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_last_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .last()
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![]);
    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test]
  fn test_last_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(0..100)
      .take(50)
      .filter(|v| v % 2 == 0)
      .last()
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // The last even number in 0..49 is 48
    assert_eq!(*result.borrow(), vec![48]);
  }

  #[rxrust_macro::test]
  fn test_last_or_emits_last_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .last_or(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![5]);
  }

  #[rxrust_macro::test]
  fn test_last_or_emits_default_when_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .last_or(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0]);
  }

  #[rxrust_macro::test]
  fn test_last_or_with_single_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::of(42).last_or(0).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_last_or_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .last_or(())
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |_v| {
        result_clone.borrow_mut().push(());
      });

    assert_eq!(*result.borrow(), vec![]);
    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test]
  fn test_last_or_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(0..100)
      .take(5)
      .filter(|v| *v > 10) // This will filter out all values
      .last_or(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should emit default value since filter removed all items
    assert_eq!(*result.borrow(), vec![0]);
  }
}
