//! Distinct operator implementation
//!
//! This module contains the `Distinct` and `DistinctKey` operators, which
//! filtering items emitted by the source observable.

use std::{collections::HashSet, hash::Hash};

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Distinct operator: Emits only items that have not been seen before.
///
/// This operator uses a `HashSet` to store seen items, filtering out
/// duplicates.
///
/// It requires the item type to implement `Eq`, `Hash` and `Clone`.
#[derive(Clone)]
pub struct Distinct<S>(pub S);

impl<S> ObservableType for Distinct<S>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, C, Unsub> CoreObservable<C> for Distinct<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<DistinctObserver<C::Inner, <S as ObservableType>::Item<'a>>>,
      Item<'a>: Eq + Hash + Clone,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.transform(DistinctObserver::new);
    self.0.subscribe(observer)
  }
}

/// DistinctObserver wrapper for filtering duplicates
pub struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

impl<O, Item> DistinctObserver<O, Item> {
  pub fn new(observer: O) -> Self { Self { observer, seen: HashSet::new() } }
}

impl<O, Item, Err> Observer<Item, Err> for DistinctObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: Eq + Hash + Clone,
{
  fn next(&mut self, value: Item) {
    if !self.seen.contains(&value) {
      self.seen.insert(value.clone());
      self.observer.next(value);
    }
  }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

/// DistinctKey operator: Emits items where the keys derived have not been seen
/// before.
///
/// This operator uses a `HashSet` to store seen keys, filtering out items with
/// duplicate keys.
///
/// It requires the key type to implement `Eq`, `Hash` and `Clone`.
#[derive(Clone)]
pub struct DistinctKey<S, F> {
  pub(crate) source: S,
  pub(crate) key_selector: F,
}

impl<S, F> ObservableType for DistinctKey<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, C, Key> CoreObservable<C> for DistinctKey<S, F>
where
  C: Context,
  S: CoreObservable<C::With<DistinctKeyObserver<C::Inner, F, Key>>>,
  F: for<'a> Fn(&S::Item<'a>) -> Key,
  Key: Eq + Hash + Clone,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DistinctKey { source, key_selector } = self;
    let observer = context.transform(|observer| DistinctKeyObserver::new(observer, key_selector));
    source.subscribe(observer)
  }
}

/// DistinctKeyObserver wrapper for filtering duplicates by key
pub struct DistinctKeyObserver<O, F, Key> {
  observer: O,
  key_selector: F,
  seen: HashSet<Key>,
}

impl<O, F, Key> DistinctKeyObserver<O, F, Key> {
  pub fn new(observer: O, key_selector: F) -> Self {
    Self { observer, key_selector, seen: HashSet::new() }
  }
}

impl<O, F, Key, Item, Err> Observer<Item, Err> for DistinctKeyObserver<O, F, Key>
where
  O: Observer<Item, Err>,
  F: Fn(&Item) -> Key,
  Key: Eq + Hash + Clone,
{
  fn next(&mut self, value: Item) {
    let key = (self.key_selector)(&value);
    if !self.seen.contains(&key) {
      self.seen.insert(key);
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
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    Local::from_iter(0..20)
      .map(|v| v % 5)
      .distinct()
      .subscribe(move |v| x_c.borrow_mut().push(v));
    assert_eq!(&*x.borrow(), &[0, 1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn distinct_key() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    Local::from_iter(vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)])
      .distinct_key(|tup: &(i32, i32)| tup.0)
      .subscribe(move |v| x_c.borrow_mut().push(v));

    assert_eq!(&*x.borrow(), &[(1, 2), (2, 2), (3, 2)]);
  }
}
