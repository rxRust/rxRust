use crate::{impl_local_shared_both, prelude::*};
use std::{cmp::Eq, collections::HashSet, hash::Hash};

#[derive(Clone)]
pub struct DistinctOp<S> {
  pub(crate) source: S,
}

impl<S: Observable> Observable for DistinctOp<S> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> DistinctOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DistinctObserver {
      observer: $observer,
      seen: HashSet::new(),
    })
  }
  where
    S: @ctx::Observable,
    S::Item: Eq + Hash + Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)
}
struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

impl<O, Item, Err> Observer for DistinctObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Hash + Eq + Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if !self.seen.contains(&value) {
      self.seen.insert(value.clone());
      self.observer.next(value);
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[derive(Clone)]
pub struct DistinctKeyOp<S, F> {
  pub(crate) source: S,
  pub(crate) key: F,
}

impl<S: Observable, F> Observable for DistinctKeyOp<S, F> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, F, K> DistinctKeyOp<S, F>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DistinctKeyObserver {
      observer: $observer,
      key: $self.key,
      seen: HashSet::new(),
    })
  }
  where
    S: @ctx::Observable,
    F: Fn(&S::Item) -> K
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static),
    K: Eq + Hash + Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static),
}
struct DistinctKeyObserver<O, F, K> {
  observer: O,
  key: F,
  seen: HashSet<K>,
}

impl<O, F, K, Item, Err> Observer for DistinctKeyObserver<O, F, K>
where
  O: Observer<Item = Item, Err = Err>,
  K: Hash + Eq + Clone,
  F: Fn(&Item) -> K,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    let key = (self.key)(&value);
    if !self.seen.contains(&key) {
      self.seen.insert(key);
      self.observer.next(value);
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[derive(Clone)]
pub struct DistinctUntilChangedOp<S> {
  pub(crate) source: S,
}

impl<S: Observable> Observable for DistinctUntilChangedOp<S> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> DistinctUntilChangedOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DistinctUntilChangedObserver {
      observer: $observer,
      last: None,
    })
  }
  where
    S: @ctx::Observable,
    S::Item: Eq + Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)
}
struct DistinctUntilChangedObserver<O, Item> {
  observer: O,
  last: Option<Item>,
}

impl<O, Item, Err> Observer for DistinctUntilChangedObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Eq + Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if self.last.is_none() || self.last.as_ref().unwrap() != &value {
      self.last = Some(value.clone());
      self.observer.next(value);
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[derive(Clone)]
pub struct DistinctUntilKeyChangedOp<S, F> {
  pub(crate) source: S,
  pub(crate) key: F,
}

impl<S: Observable, F> Observable for DistinctUntilKeyChangedOp<S, F> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, F, K> DistinctUntilKeyChangedOp<S, F>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DistinctUntilKeyChangedObserver {
      observer: $observer,
      key: $self.key,
      last: None,
    })
  }
  where
    F: Fn(&S::Item) -> K
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static),
    S: @ctx::Observable,
    K: Eq,
    S::Item: Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)
}
struct DistinctUntilKeyChangedObserver<O, Item, F> {
  observer: O,
  key: F,
  last: Option<Item>,
}

impl<O, F, K, Item, Err> Observer
  for DistinctUntilKeyChangedObserver<O, Item, F>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Clone,
  K: Eq,
  F: Fn(&Item) -> K,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if self.last.is_none()
      || (self.key)(self.last.as_ref().unwrap()) != (self.key)(&value)
    {
      self.last = Some(value.clone());
      self.observer.next(value);
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
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
      .subscribe(move |v| x.borrow_mut().push(v))
      .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[0, 1, 2, 3, 4]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared() {
    observable::from_iter(0..10)
      .distinct()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_distinct);

  fn bench_distinct(b: &mut bencher::Bencher) { b.iter(smoke); }

  #[test]
  fn distinct_until_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(&[1, 2, 2, 1, 2, 3])
      .map(|v| v % 5)
      .distinct_until_changed()
      .subscribe(move |v| x.borrow_mut().push(v))
      .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[1, 2, 1, 2, 3]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn distinct_until_changed_shared() {
    observable::from_iter(0..10)
      .distinct_until_changed()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench2() { do_bench_distinct_until_changed(); }
  benchmark_group!(
    do_bench_distinct_until_changed,
    bench_distinct_until_changed
  );

  fn bench_distinct_until_changed(b: &mut bencher::Bencher) { b.iter(smoke); }

  #[test]
  fn distinct_until_key_changed() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(
      vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)].into_iter(),
    )
    .map(|v| v)
    .distinct_until_key_changed(|tup: &(i32, i32)| tup.0)
    .subscribe(move |v| x.borrow_mut().push(v))
    .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[(1, 2), (2, 2), (1, 1), (2, 2), (3, 2)]);
  }

  #[test]
  fn distinct_key() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(
      vec![(1, 2), (2, 2), (2, 1), (1, 1), (2, 2), (3, 2)].into_iter(),
    )
    .map(|v| v)
    .distinct_key(|tup: &(i32, i32)| tup.0)
    .subscribe(move |v| x.borrow_mut().push(v))
    .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[(1, 2), (2, 2), (3, 2)]);
  }
}
