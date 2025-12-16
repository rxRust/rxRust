//! Lifecycle operators: OnError and OnComplete
//!
//! These operators provide lifecycle hooks for reacting to error and completion
//! events.

use std::convert::Infallible;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

// ==================== OnError ====================

/// OnError operator that executes a callback when an error occurs
#[derive(Clone)]
pub struct OnError<S, F> {
  pub source: S,
  pub callback: F,
}

impl<S, F> OnError<S, F> {
  /// Create a new OnError operator
  pub fn new(source: S, callback: F) -> Self { Self { source, callback } }
}

impl<S, F> ObservableType for OnError<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer wrapper for OnError that executes the callback on error
#[derive(Clone)]
pub struct OnErrorObserver<O, F> {
  observer: O,
  callback: F,
}

impl<O, F, Item, Err> Observer<Item, Err> for OnErrorObserver<O, F>
where
  O: Observer<Item, Infallible>,
  F: FnOnce(Err),
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, err: Err) { (self.callback)(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C> CoreObservable<C> for OnError<S, F>
where
  C: Context,
  C::Inner: for<'a> Observer<S::Item<'a>, Infallible>,
  S: CoreObservable<C::With<OnErrorObserver<C::Inner, F>>>,
  F: FnOnce(S::Err),
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let OnError { source, callback } = self;
    let wrapped = context.transform(|observer| OnErrorObserver { observer, callback });
    source.subscribe(wrapped)
  }
}

// ==================== OnComplete ====================

/// OnComplete operator that executes a callback when the stream completes
#[derive(Clone)]
pub struct OnComplete<S, F> {
  pub source: S,
  pub callback: F,
}

impl<S, F> OnComplete<S, F> {
  /// Create a new OnComplete operator
  pub fn new(source: S, callback: F) -> Self { Self { source, callback } }
}

impl<S, F> ObservableType for OnComplete<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer wrapper for OnComplete that executes the callback on completion
pub struct OnCompleteObserver<O, F> {
  observer: O,
  callback: F,
}

impl<O, F, Item, Err> Observer<Item, Err> for OnCompleteObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnOnce(),
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) {
    (self.callback)();
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C> CoreObservable<C> for OnComplete<S, F>
where
  C: Context,
  S: CoreObservable<C::With<OnCompleteObserver<C::Inner, F>>>,
  F: FnOnce(),
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let OnComplete { source, callback } = self;
    let wrapped = context.transform(|observer| OnCompleteObserver { observer, callback });
    source.subscribe(wrapped)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_on_complete() {
    let completed_calls = Rc::new(RefCell::new(Vec::new()));
    let completed_calls_clone = completed_calls.clone();

    Local::of(42)
      .on_complete(move || {
        completed_calls_clone
          .borrow_mut()
          .push("completed")
      })
      .subscribe(|v| {
        assert_eq!(v, 42);
      });

    assert_eq!(*completed_calls.borrow(), vec!["completed"]);
  }

  #[rxrust_macro::test]
  fn test_chained_lifecycle_operators() {
    let complete_calls = Rc::new(RefCell::new(Vec::new()));
    let complete_calls_clone1 = complete_calls.clone();
    let complete_calls_clone2 = complete_calls.clone();

    Local::of(42)
      .on_complete(move || {
        complete_calls_clone1
          .borrow_mut()
          .push("First complete")
      })
      .on_complete(move || {
        complete_calls_clone2
          .borrow_mut()
          .push("Second complete")
      })
      .subscribe(|v| {
        assert_eq!(v, 42);
      });

    assert_eq!(*complete_calls.borrow(), vec!["First complete", "Second complete"]);
  }

  #[rxrust_macro::test]
  fn test_complete_with_normal_stream() {
    let completed = Rc::new(RefCell::new(false));
    let values = Rc::new(RefCell::new(Vec::new()));

    let completed_clone = completed.clone();
    let values_clone = values.clone();

    Local::of(42)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        values_clone.borrow_mut().push(v);
      });

    assert_eq!(*values.borrow(), vec![42]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_on_error_with_no_error_stream() {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let values = Rc::new(RefCell::new(Vec::new()));

    let errors_clone = errors.clone();
    let values_clone = values.clone();

    Local::of(42)
      .on_error(move |e| errors_clone.borrow_mut().push(e))
      .subscribe(move |v| {
        values_clone.borrow_mut().push(v);
      });

    assert_eq!(*values.borrow(), vec![42]);
    assert_eq!(errors.borrow().len(), 0); // No error occurred
  }

  #[rxrust_macro::test]
  fn test_on_complete_with_map() {
    let completed_calls = Rc::new(RefCell::new(Vec::new()));
    let values = Rc::new(RefCell::new(Vec::new()));

    let completed_calls_clone = completed_calls.clone();
    let values_clone = values.clone();

    Local::of(21)
      .map(|x| x * 2)
      .on_complete(move || {
        completed_calls_clone
          .borrow_mut()
          .push("mapped completed")
      })
      .subscribe(move |v| {
        values_clone.borrow_mut().push(v);
      });

    assert_eq!(*values.borrow(), vec![42]); // Mapped value
    assert_eq!(*completed_calls.borrow(), vec!["mapped completed"]);
  }

  #[rxrust_macro::test]
  fn test_multiple_on_complete_handlers() {
    let complete_calls = Rc::new(RefCell::new(Vec::new()));
    let complete_calls_clone1 = complete_calls.clone();
    let complete_calls_clone2 = complete_calls.clone();
    let complete_calls_clone3 = complete_calls.clone();

    Local::of(42)
      .on_complete(move || {
        complete_calls_clone1
          .borrow_mut()
          .push("First complete")
      })
      .on_complete(move || {
        complete_calls_clone2
          .borrow_mut()
          .push("Second complete")
      })
      .on_complete(move || {
        complete_calls_clone3
          .borrow_mut()
          .push("Third complete")
      })
      .subscribe(|v| {
        assert_eq!(v, 42);
      });

    assert_eq!(
      *complete_calls.borrow(),
      vec!["First complete", "Second complete", "Third complete"]
    );
  }

  #[rxrust_macro::test]
  fn test_combined_error_and_complete() {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let values = Rc::new(RefCell::new(Vec::new()));

    let errors_clone = errors.clone();
    let completed_clone = completed.clone();
    let values_clone = values.clone();

    // Test that both handlers can be attached to a normal stream
    Local::of(42)
      .on_error(move |e| errors_clone.borrow_mut().push(e))
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        values_clone.borrow_mut().push(v);
      });

    assert_eq!(*values.borrow(), vec![42]);
    assert_eq!(errors.borrow().len(), 0); // No error occurred
    assert!(*completed.borrow()); // Completion still happens
  }
}
