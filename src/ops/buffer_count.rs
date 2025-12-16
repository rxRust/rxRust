//! BufferCount operator implementation.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// BufferCount operator.
///
/// Collects items into a `Vec` until `count` is reached, then emits the buffer.
/// When the source completes, any remaining items are emitted (if non-empty).
#[derive(Clone)]
pub struct BufferCount<S> {
  pub source: S,
  pub count: usize,
}

impl<S: ObservableType> ObservableType for BufferCount<S> {
  type Item<'a>
    = Vec<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer for BufferCount.
pub struct BufferCountObserver<O, Item> {
  observer: Option<O>,
  buffer: Vec<Item>,
  count: usize,
}

impl<O, Item, Err> Observer<Item, Err> for BufferCountObserver<O, Item>
where
  O: Observer<Vec<Item>, Err>,
{
  fn next(&mut self, v: Item) {
    if let Some(ref mut observer) = self.observer {
      self.buffer.push(v);
      if self.buffer.len() >= self.count {
        observer.next(std::mem::take(&mut self.buffer));
      }
    }
  }

  fn error(self, e: Err) {
    if let Some(observer) = self.observer {
      observer.error(e);
    }
  }

  fn complete(mut self) {
    if let Some(mut observer) = self.observer.take() {
      if !self.buffer.is_empty() {
        observer.next(std::mem::take(&mut self.buffer));
      }
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

impl<S, C, Unsub> CoreObservable<C> for BufferCount<S>
where
  C: Context,
  S: ObservableType,
  S: for<'a> CoreObservable<
      C::With<BufferCountObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let BufferCount { source, count } = self;
    let wrapped = context.transform(|observer| BufferCountObserver {
      observer: Some(observer),
      buffer: Vec::new(),
      count,
    });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_buffer_count_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .buffer_count(2)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3, 4], vec![5]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_count_exact_multiple() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4])
      .buffer_count(2)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3, 4]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_count_empty_source() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .buffer_count(2)
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert!(result.borrow().is_empty());
  }

  #[rxrust_macro::test]
  fn test_buffer_count_completes() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3])
      .buffer_count(2)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_buffer_count_error() {
    let error = Rc::new(RefCell::new(None));
    let error_clone = error.clone();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let subject = Local::subject::<i32, String>();
    subject
      .clone()
      .buffer_count(2)
      .on_error(move |e| *error_clone.borrow_mut() = Some(e))
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    subject.error("test error".to_string());

    assert_eq!(*error.borrow(), Some("test error".to_string()));
    assert!(result.borrow().is_empty());
  }

  #[rxrust_macro::test]
  fn test_buffer_count_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5, 6])
      .buffer_count(2)
      .map(|v: Vec<i32>| v.iter().sum::<i32>())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![3, 7, 11]);
  }
}
