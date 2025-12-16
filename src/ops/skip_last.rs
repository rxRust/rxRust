//! SkipLast operator implementation
//!
//! This module contains the SkipLast operator, which skips the last `count`
//! values emitted by the source Observable.

use std::collections::VecDeque;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// SkipLast operator: Skips the last `count` values from the source observable
///
/// This operator buffers up to `count` values. When a new value arrives, if
/// the buffer is full, it emits the oldest value and adds the new one.
/// When the source completes, the buffered values (the last `count` items)
/// are discarded.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip_last(2);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3]);
/// ```
#[derive(Clone)]
pub struct SkipLast<S> {
  pub source: S,
  pub count: usize,
}

impl<S: ObservableType> ObservableType for SkipLast<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// SkipLastObserver wrapper for buffering and skipping the last `count` values
///
/// This observer wraps another observer and buffers up to `count` values.
/// When a new value arrives and the buffer is full, the oldest value is
/// emitted.
pub struct SkipLastObserver<O, Item> {
  observer: O,
  count_down: usize,
  queue: VecDeque<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for SkipLastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, v: Item) {
    if self.count_down > 0 {
      self.count_down -= 1;
      self.queue.push_back(v);
    } else {
      // Optimization: Pop before push to avoid reallocation if buffer is at capacity.
      // If queue is empty (count == 0), pass through directly.
      if let Some(old) = self.queue.pop_front() {
        self.queue.push_back(v);
        self.observer.next(old);
      } else {
        self.observer.next(v);
      }
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C, Unsub> CoreObservable<C> for SkipLast<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<SkipLastObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SkipLast { source, count } = self;
    let wrapped = context.transform(|observer| SkipLastObserver {
      observer,
      count_down: count,
      queue: VecDeque::new(),
    });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_skip_last_base_function() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(0..10)
      .skip_last(5)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0, 1, 2, 3, 4]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_skip_last_empty_result() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(0..10)
      .skip_last(11)
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
  fn test_skip_last_chaining() {
    let mut nc1 = 0;
    let mut nc2 = 0;

    {
      let skip_last5 = Local::from_iter(0..100).skip_last(5);
      let f1 = skip_last5.clone();
      let f2 = skip_last5;

      f1.skip_last(5).subscribe(|_| nc1 += 1);
      f2.skip_last(5).subscribe(|_| nc2 += 1);
    }

    assert_eq!(nc1, 90);
    assert_eq!(nc2, 90);
  }

  #[rxrust_macro::test]
  fn test_skip_last_with_zero_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .skip_last(0)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should emit all values when count is 0
    assert_eq!(*result.borrow(), vec![1, 2, 3, 4, 5]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_skip_last_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .skip_last(5)
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
