//! DistinctUntilChanged operator implementation
//!
//! This module contains the `DistinctUntilChanged` and
//! `DistinctUntilKeyChanged` operators, which filter consecutive duplicate
//! items emitted by the source observable.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  prelude::Subscription,
};

/// DistinctUntilChanged operator: Emits items only if they are different from
/// the previous item.
///
/// This operator suppresses consecutive duplicates.
///
/// It requires the item type to implement `PartialEq` and `Clone`.
#[derive(Clone)]
pub struct DistinctUntilChanged<S>(pub S);

impl<S> ObservableType for DistinctUntilChanged<S>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, C, Unsub> CoreObservable<C> for DistinctUntilChanged<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<DistinctUntilChangedObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.transform(DistinctUntilChangedObserver::new);
    self.0.subscribe(observer)
  }
}

/// DistinctUntilChangedObserver wrapper for filtering consecutive duplicates
pub struct DistinctUntilChangedObserver<O, Item> {
  observer: O,
  last: Option<Item>,
}

impl<O, Item> DistinctUntilChangedObserver<O, Item> {
  pub fn new(observer: O) -> Self { Self { observer, last: None } }
}

impl<O, Item, Err> Observer<Item, Err> for DistinctUntilChangedObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: PartialEq + Clone,
{
  fn next(&mut self, value: Item) {
    if self.last.as_ref() != Some(&value) {
      self.last = Some(value.clone());
      self.observer.next(value);
    }
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

/// DistinctUntilKeyChanged operator: Emits items where the keys derived from
/// consecutive items are different.
///
/// This operator suppresses consecutive items that have the same key.
///
/// It requires the key type to implement `PartialEq`.
#[derive(Clone)]
pub struct DistinctUntilKeyChanged<S, F> {
  pub(crate) source: S,
  pub(crate) key_selector: F,
}

impl<S, F> ObservableType for DistinctUntilKeyChanged<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, C, Key, Unsub> CoreObservable<C> for DistinctUntilKeyChanged<S, F>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<DistinctUntilKeyChangedObserver<C::Inner, F, Key>>,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
  F: for<'a> Fn(&<S as ObservableType>::Item<'a>) -> Key,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DistinctUntilKeyChanged { source, key_selector } = self;
    let observer =
      context.transform(|observer| DistinctUntilKeyChangedObserver::new(observer, key_selector));
    source.subscribe(observer)
  }
}

/// DistinctUntilKeyChangedObserver wrapper for filtering duplicates by key
pub struct DistinctUntilKeyChangedObserver<O, F, Key> {
  observer: O,
  key_selector: F,
  last_key: Option<Key>,
}

impl<O, F, Key> DistinctUntilKeyChangedObserver<O, F, Key> {
  pub fn new(observer: O, key_selector: F) -> Self {
    Self { observer, key_selector, last_key: None }
  }
}

impl<O, F, Key, Item, Err> Observer<Item, Err> for DistinctUntilKeyChangedObserver<O, F, Key>
where
  O: Observer<Item, Err>,
  F: Fn(&Item) -> Key,
  Key: PartialEq,
{
  fn next(&mut self, value: Item) {
    let key = (self.key_selector)(&value);
    if self.last_key.as_ref() != Some(&key) {
      self.last_key = Some(key);
      self.observer.next(value);
    }
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

#[cfg(test)]
mod tests {

  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn smoke_distinct_until_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    Local::from_iter(&[1, 2, 2, 1, 2, 3])
      .map(|v| v % 5)
      .distinct_until_changed()
      .subscribe(move |v| x_c.borrow_mut().push(v));
    assert_eq!(&*x.borrow(), &[1, 2, 1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn smoke_distinct_until_key_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    Local::from_iter(vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)])
      .distinct_until_key_changed(|tup: &(i32, i32)| tup.0)
      .subscribe(move |v| x_c.borrow_mut().push(v));

    // Keys: 1, 2, 2, 1, 2, 3
    // Emitted keys: 1, 2, 1, 2, 3
    // Emitted items: (1,2), (2,2), (1,1), (2,2), (3,2)
    // Note: (2,2) and (2,1) have same key 2. So (2,1) is skipped.
    assert_eq!(&*x.borrow(), &[(1, 2), (2, 2), (1, 1), (2, 2), (3, 2)]);
  }

  #[rxrust_macro::test]
  fn distinct_until_key_changed_no_clone() {
    #[derive(PartialEq, Debug)]
    struct NoClone(i32);

    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();

    // Using from_iter requires Clone typically, so we use from_fn or just array?
    // Local::from_iter takes IntoIterator. If we pass vec of NoClone, it might
    // work? But map might require Clone if it was cloning?
    // Let's rely on the fact that we moved items.

    let source = Local::from_iter(vec![NoClone(1), NoClone(2), NoClone(2), NoClone(3)]);

    source
      .distinct_until_key_changed(|v: &NoClone| v.0)
      .subscribe(move |v| x_c.borrow_mut().push(v));

    assert_eq!(&*x.borrow(), &[NoClone(1), NoClone(2), NoClone(3)]);
  }
}
