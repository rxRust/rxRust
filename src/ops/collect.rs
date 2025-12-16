//! Collect operator implementation
//!
//! This module contains the Collect operator, which accumulates all emitted
//! items into a collection and emits the collection on completion.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Collect operator: Accumulates items into a collection
///
/// This operator demonstrates:
/// - Observer wrapping with accumulator state (CollectObserver)
/// - Context unpacking with `transform`
/// - Collection using `Extend` trait
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3]).collect::<Vec<_>>();
/// let mut result = None;
/// observable.subscribe(|v| {
///   result = Some(v);
/// });
/// assert_eq!(result, Some(vec![1, 2, 3]));
/// ```
#[derive(Clone)]
pub struct Collect<S, C> {
  pub source: S,
  pub collection: C,
}

/// CollectObserver wrapper for accumulating values
///
/// This observer wraps another observer and accumulates all received values
/// into a collection. The collection is emitted when the source completes.
pub struct CollectObserver<O, C> {
  observer: O,
  collection: C,
}

impl<O, C, Item, Err> Observer<Item, Err> for CollectObserver<O, C>
where
  O: Observer<C, Err>,
  C: Extend<Item>,
{
  fn next(&mut self, value: Item) { self.collection.extend(Some(value)); }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(mut self) {
    self.observer.next(self.collection);
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> ObservableType for Collect<S, C>
where
  S: ObservableType,
{
  type Item<'a>
    = C
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, C, Unsub> CoreObservable<F> for Collect<S, C>
where
  F: Context,
  S: for<'a> CoreObservable<F::With<CollectObserver<F::Inner, C>>, Unsub = Unsub>,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: F) -> Self::Unsub {
    let Collect { source, collection } = self;
    let wrapped = context.transform(|observer| CollectObserver { observer, collection });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_collect_basic() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .collect::<Vec<_>>()
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*result.borrow(), Some(vec![1, 2, 3]));
  }

  #[rxrust_macro::test(local)]
  async fn test_collect_into() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::from_iter([4, 5, 6])
      .collect_into::<Vec<_>>(vec![1, 2, 3])
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*result.borrow(), Some(vec![1, 2, 3, 4, 5, 6]));
  }

  #[rxrust_macro::test(local)]
  async fn test_collect_empty() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .collect::<Vec<_>>()
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*result.borrow(), Some(vec![]));
  }

  #[rxrust_macro::test(local)]
  async fn test_collect_error_does_not_emit_collection() {
    let result = Rc::new(RefCell::new(None));
    let error = Rc::new(RefCell::new(None));
    let result_clone = result.clone();
    let error_clone = error.clone();

    let mut subject: Local<_> = Local::subject::<i32, String>();

    subject
      .clone()
      .collect::<Vec<_>>()
      .on_error(move |e| {
        *error_clone.borrow_mut() = Some(e);
      })
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    subject.next(1);
    subject.next(2);
    subject.next(3);
    subject.error("something went wrong".to_string());

    assert!(result.borrow().is_none());
    assert_eq!(*error.borrow(), Some("something went wrong".to_string()));
  }
}
