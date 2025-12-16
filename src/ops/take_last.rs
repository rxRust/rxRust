//! TakeLast operator implementation
//!
//! This module contains the TakeLast operator, which emits only the last
//! `count` values emitted by the source Observable when the source completes.

use std::collections::VecDeque;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// TakeLast operator: Emits only the last `count` values from the source
/// observable
///
/// This operator buffers the last `count` values emitted by the source
/// observable. When the source completes, it emits all buffered values in order
/// and then completes. If the source emits fewer than `count` values, all
/// values are emitted.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).take_last(3);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![3, 4, 5]);
/// ```
#[derive(Clone)]
pub struct TakeLast<S> {
  pub source: S,
  pub count: usize,
}

impl<S: ObservableType> ObservableType for TakeLast<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// TakeLastObserver wrapper for buffering and emitting the last `count` values
///
/// This observer wraps another observer and buffers up to `count` values.
/// When the source completes, it emits all buffered values in order.
pub struct TakeLastObserver<O, Item> {
  observer: O,
  count: usize,
  queue: VecDeque<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for TakeLastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, v: Item) {
    if self.count == 0 {
      return;
    }
    if self.queue.len() == self.count {
      self.queue.pop_front();
    }
    self.queue.push_back(v);
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(mut self) {
    for value in self.queue.drain(..) {
      self.observer.next(value);
    }
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C, Unsub> CoreObservable<C> for TakeLast<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<TakeLastObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let TakeLast { source, count } = self;
    let wrapped =
      context.transform(|observer| TakeLastObserver { observer, count, queue: VecDeque::new() });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_take_last_emits_last_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take_last(3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![3, 4, 5]);
  }

  #[rxrust_macro::test]
  fn test_take_last_completes_after_emitting() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(0..100)
      .take_last(5)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![95, 96, 97, 98, 99]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_last_with_zero_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take_last(0)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should not emit any values
    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_last_with_count_greater_than_source() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .take_last(10)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should emit all values since count > source length
    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_take_last_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(0..100)
      .take_last(5)
      .take_last(3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // First take_last(5) gives [95, 96, 97, 98, 99]
    // Second take_last(3) gives [97, 98, 99]
    assert_eq!(*result.borrow(), vec![97, 98, 99]);
  }

  #[rxrust_macro::test]
  fn test_take_last_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .take_last(5)
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
