//! Materialize and Dematerialize operator implementations
//!
//! `materialize` turns every event into a [`Notification`] item;
//! `dematerialize` turns [`Notification`] items back into events.

use std::{convert::Infallible, marker::PhantomData};

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// A reified observable event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification<Item, Err> {
  /// An item was emitted
  Next(Item),
  /// The stream errored
  Error(Err),
  /// The stream completed
  Complete,
}

/// Materialize operator: Emits every event as a [`Notification`]
///
/// Errors and completion become items, after which the stream completes.
/// The resulting error type is [`Infallible`].
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 2])
///   .materialize()
///   .subscribe(|n| result.push(n));
/// assert_eq!(result, vec![Notification::Next(1), Notification::Next(2), Notification::Complete]);
/// ```
#[derive(Clone)]
pub struct Materialize<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for Materialize<S> {
  type Item<'a>
    = Notification<S::Item<'a>, S::Err>
  where
    Self: 'a;
  type Err = Infallible;
}

/// Observer that wraps events into notifications
pub struct MaterializeObserver<O> {
  observer: O,
}

impl<O, Item, Err> Observer<Item, Err> for MaterializeObserver<O>
where
  O: Observer<Notification<Item, Err>, Infallible>,
{
  fn next(&mut self, v: Item) { self.observer.next(Notification::Next(v)); }

  fn error(self, e: Err) {
    let mut observer = self.observer;
    observer.next(Notification::Error(e));
    observer.complete();
  }

  fn complete(self) {
    let mut observer = self.observer;
    observer.next(Notification::Complete);
    observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> CoreObservable<C> for Materialize<S>
where
  C: Context,
  S: CoreObservable<C::With<MaterializeObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped = context.transform(|observer| MaterializeObserver { observer });
    self.source.subscribe(wrapped)
  }
}

/// Dematerialize operator: Replays [`Notification`] items as real events
///
/// The source must be infallible; its items are converted into
/// notifications. The first `Error` or `Complete` notification terminates
/// the stream and unsubscribes the source.
///
/// # Examples
///
/// ```
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// let error = Rc::new(RefCell::new(None));
/// let sink = error.clone();
/// Local::from_iter(vec![
///   Notification::Next(1),
///   Notification::Error("boom".to_string()),
///   Notification::Next(2),
/// ])
/// .dematerialize()
/// .on_error(move |e| *sink.borrow_mut() = Some(e))
/// .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1]);
/// assert_eq!(error.borrow().as_deref(), Some("boom"));
/// ```
pub struct Dematerialize<S, Item, Err> {
  pub source: S,
  _marker: PhantomData<fn() -> (Item, Err)>,
}

impl<S: Clone, Item, Err> Clone for Dematerialize<S, Item, Err> {
  fn clone(&self) -> Self { Self { source: self.source.clone(), _marker: PhantomData } }
}

impl<S, Item, Err> Dematerialize<S, Item, Err> {
  /// Wraps a source of notifications.
  pub fn new(source: S) -> Self { Self { source, _marker: PhantomData } }
}

impl<S, Item, Err> ObservableType for Dematerialize<S, Item, Err>
where
  S: ObservableType,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Err;
}

/// Observer that unwraps notifications into events
pub struct DematerializeObserver<O, Item, Err> {
  observer: Option<O>,
  _marker: PhantomData<fn() -> (Item, Err)>,
}

impl<O, SrcItem, Item, Err> Observer<SrcItem, Infallible> for DematerializeObserver<O, Item, Err>
where
  O: Observer<Item, Err>,
  SrcItem: Into<Notification<Item, Err>>,
{
  fn next(&mut self, v: SrcItem) {
    match v.into() {
      Notification::Next(item) => {
        if let Some(observer) = self.observer.as_mut() {
          observer.next(item);
        }
      }
      Notification::Error(e) => {
        if let Some(observer) = self.observer.take() {
          observer.error(e);
        }
      }
      Notification::Complete => {
        if let Some(observer) = self.observer.take() {
          observer.complete();
        }
      }
    }
  }

  fn error(self, e: Infallible) { match e {} }

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

impl<S, C, Item, Err> CoreObservable<C> for Dematerialize<S, Item, Err>
where
  C: Context,
  S: CoreObservable<C::With<DematerializeObserver<C::Inner, Item, Err>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped = context.transform(|observer| DematerializeObserver {
      observer: Some(observer),
      _marker: PhantomData,
    });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::Notification;
  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_materialize_items_and_complete() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1, 2])
      .materialize()
      .subscribe(move |n| result_c.borrow_mut().push(n));

    assert_eq!(
      *result.borrow(),
      vec![Notification::Next(1), Notification::Next(2), Notification::Complete]
    );
  }

  #[rxrust_macro::test]
  fn test_materialize_error_becomes_item() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .materialize()
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |n| result_c.borrow_mut().push(n));

    assert_eq!(*result.borrow(), vec![Notification::Error("boom".to_string())]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_dematerialize_replays_events() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter(vec![
      Notification::<i32, String>::Next(1),
      Notification::Next(2),
      Notification::Complete,
      Notification::Next(3),
    ])
    .dematerialize()
    .on_error(|_| unreachable!())
    .on_complete(move || *completed_c.borrow_mut() = true)
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_dematerialize_error_notification() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::from_iter(vec![Notification::<i32, String>::Error("boom".to_string())])
      .dematerialize()
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(error.borrow().as_deref(), Some("boom"));
  }

  #[rxrust_macro::test]
  fn test_round_trip() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1, 2, 3])
      .materialize()
      .dematerialize()
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }
}
