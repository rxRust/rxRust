use crate::prelude::*;
use std::{cmp::Eq, collections::HashSet, hash::Hash};

#[derive(Clone)]
pub struct DistinctOp<S> {
  pub(crate) source: S,
}

impl<Item, Err, O, S> Observable<Item, Err, O> for DistinctOp<S>
where
  S: Observable<Item, Err, DistinctObserver<O, Item>>,
  O: Observer<Item, Err>,
  Item: Eq + Hash + Clone,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(DistinctObserver { observer, seen: HashSet::new() })
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for DistinctOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for DistinctObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: Hash + Eq + Clone,
{
  fn next(&mut self, value: Item) {
    if !self.seen.contains(&value) {
      self.seen.insert(value.clone());
      self.observer.next(value);
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[derive(Clone)]
pub struct DistinctKeyOp<S, F> {
  pub(crate) source: S,
  pub(crate) key: F,
}

impl<Item, Err, O, S, F, K> Observable<Item, Err, O> for DistinctKeyOp<S, F>
where
  S: Observable<Item, Err, DistinctKeyObserver<O, F, K>>,
  O: Observer<Item, Err>,
  F: Fn(&Item) -> K,
  K: Eq + Hash + Clone,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(DistinctKeyObserver {
      observer,
      key: self.key,
      seen: HashSet::new(),
    })
  }
}

impl<Item, Err, S, F> ObservableExt<Item, Err> for DistinctKeyOp<S, F> where
  S: ObservableExt<Item, Err>
{
}
pub struct DistinctKeyObserver<O, F, K> {
  observer: O,
  key: F,
  seen: HashSet<K>,
}

impl<O, F, K, Item, Err> Observer<Item, Err> for DistinctKeyObserver<O, F, K>
where
  O: Observer<Item, Err>,
  K: Hash + Eq + Clone,
  F: Fn(&Item) -> K,
{
  fn next(&mut self, value: Item) {
    let key = (self.key)(&value);
    if !self.seen.contains(&key) {
      self.seen.insert(key);
      self.observer.next(value);
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[derive(Clone)]
pub struct DistinctUntilChangedOp<S> {
  pub(crate) source: S,
}

impl<Item, Err, O, S> Observable<Item, Err, O> for DistinctUntilChangedOp<S>
where
  S: Observable<Item, Err, DistinctUntilChangedObserver<O, Item>>,
  O: Observer<Item, Err>,
  Item: PartialEq + Clone,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(DistinctUntilChangedObserver { observer, last: None })
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for DistinctUntilChangedOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct DistinctUntilChangedObserver<O, Item> {
  observer: O,
  last: Option<Item>,
}

impl<O, Item, Err> Observer<Item, Err> for DistinctUntilChangedObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: PartialEq + Clone,
{
  fn next(&mut self, value: Item) {
    if self.last.is_none() || self.last.as_ref().unwrap() != &value {
      self.last = Some(value.clone());
      self.observer.next(value);
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[derive(Clone)]
pub struct DistinctUntilKeyChangedOp<S, F> {
  pub(crate) source: S,
  pub(crate) key: F,
}

impl<Item, Err, O, S, F, K> Observable<Item, Err, O>
  for DistinctUntilKeyChangedOp<S, F>
where
  S: Observable<Item, Err, DistinctUntilKeyChangedObserver<O, F, Item>>,
  O: Observer<Item, Err>,
  K: Eq,
  Item: Clone,
  F: Fn(&Item) -> K,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(DistinctUntilKeyChangedObserver {
        observer,
        key: self.key,
        last: None,
      })
  }
}

impl<Item, Err, S, F> ObservableExt<Item, Err>
  for DistinctUntilKeyChangedOp<S, F>
where
  S: ObservableExt<Item, Err>,
{
}
pub struct DistinctUntilKeyChangedObserver<O, F, Item> {
  observer: O,
  key: F,
  last: Option<Item>,
}

impl<O, F, K, Item, Err> Observer<Item, Err>
  for DistinctUntilKeyChangedObserver<O, F, Item>
where
  O: Observer<Item, Err>,
  Item: Clone,
  K: Eq,
  F: Fn(&Item) -> K,
{
  fn next(&mut self, value: Item) {
    if self.last.is_none()
      || (self.key)(self.last.as_ref().unwrap()) != (self.key)(&value)
    {
      self.last = Some(value.clone());
      self.observer.next(value);
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(0..20)
      .map(|v| v % 5)
      .distinct()
      .subscribe(move |v| x.borrow_mut().push(v));
    assert_eq!(&*x_c.borrow(), &[0, 1, 2, 3, 4]);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_distinct);

  fn bench_distinct(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }

  #[test]
  fn distinct_until_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(&[1, 2, 2, 1, 2, 3])
      .map(|v| v % 5)
      .distinct_until_changed()
      .subscribe(move |v| x.borrow_mut().push(v));
    assert_eq!(&*x_c.borrow(), &[1, 2, 1, 2, 3]);
  }

  #[test]
  fn bench2() {
    do_bench_distinct_until_changed();
  }
  benchmark_group!(
    do_bench_distinct_until_changed,
    bench_distinct_until_changed
  );

  fn bench_distinct_until_changed(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }

  #[test]
  fn distinct_until_key_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)])
      .map(|v| v)
      .distinct_until_key_changed(|tup: &(i32, i32)| tup.0)
      .subscribe(move |v| x.borrow_mut().push(v));
    assert_eq!(&*x_c.borrow(), &[(1, 2), (2, 2), (1, 1), (2, 2), (3, 2)]);
  }

  #[test]
  fn distinct_key() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)])
      .distinct_key(|tup: &(i32, i32)| tup.0)
      .subscribe(move |v| x.borrow_mut().push(v));

    assert_eq!(&*x_c.borrow(), &[(1, 2), (2, 2), (3, 2)]);
  }
}
