//! Reduce operator implementation
//!
//! This module contains the `Reduce` operator, which applies an accumulator
//! function over the source Observable and emits the final accumulated value
//! when the source completes.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Defines the strategy for executing the reduction logic (seeded vs.
/// unseeded).
pub trait ReduceStrategy<Acc, Item> {
  /// Applies the reduction logic.
  ///
  /// # Arguments
  /// * `acc` - The current accumulated value. `None` indicates the first
  ///   element for unseeded reduction.
  /// * `value` - The next item emitted by the source.
  fn apply(&mut self, acc: Option<Acc>, value: Item) -> Option<Acc>;
}

/// Strategy for `reduce` (unseeded).
///
/// Uses the first emitted value as the initial accumulator.
/// Requires `Acc` to be the same type as `Item`.
#[derive(Clone)]
pub struct ReduceFn<F>(pub F);

impl<F, Item> ReduceStrategy<Item, Item> for ReduceFn<F>
where
  F: FnMut(Item, Item) -> Item,
{
  fn apply(&mut self, acc: Option<Item>, value: Item) -> Option<Item> {
    match acc {
      Some(acc) => Some((self.0)(acc, value)),
      None => Some(value),
    }
  }
}

/// Strategy for `reduce_initial` (seeded).
///
/// Starts with an explicit initial value.
#[derive(Clone)]
pub struct ReduceInitialFn<F>(pub F);

impl<F, Acc, Item> ReduceStrategy<Acc, Item> for ReduceInitialFn<F>
where
  F: FnMut(Acc, Item) -> Acc,
{
  fn apply(&mut self, acc: Option<Acc>, value: Item) -> Option<Acc> {
    // acc is always Some(...) because it is initialized with the seed.
    acc.map(|a| (self.0)(a, value))
  }
}

/// The `Reduce` operator.
///
/// Applies an accumulator function over the source Observable and emits the
/// final result.
///
/// This struct is created by the `reduce` and `reduce_initial` methods on
/// `Observable`.
#[derive(Clone)]
pub struct Reduce<S, Strategy, Acc> {
  /// The source Observable.
  pub source: S,
  /// The reduction strategy (encapsulating the accumulator function).
  pub strategy: Strategy,
  /// The initial accumulated value (None for unseeded reduction).
  pub initial: Option<Acc>,
}

impl<S, Strategy, Acc> ObservableType for Reduce<S, Strategy, Acc>
where
  S: ObservableType,
{
  type Item<'a>
    = Acc
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, Strategy, C, Acc> CoreObservable<C> for Reduce<S, Strategy, Acc>
where
  C: Context,
  S: CoreObservable<C::With<ReduceObserver<C::Inner, Strategy, Acc>>>,
  Strategy: for<'a> ReduceStrategy<Acc, S::Item<'a>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Reduce { source, strategy, initial } = self;
    let wrapped = context.transform(|observer| ReduceObserver { observer, strategy, acc: initial });
    source.subscribe(wrapped)
  }
}

/// Observer implementation for the Reduce operator.
pub struct ReduceObserver<O, Strategy, Acc> {
  observer: O,
  strategy: Strategy,
  acc: Option<Acc>,
}

impl<O, Strategy, Acc, Item, Err> Observer<Item, Err> for ReduceObserver<O, Strategy, Acc>
where
  O: Observer<Acc, Err>,
  Strategy: ReduceStrategy<Acc, Item>,
{
  fn next(&mut self, value: Item) { self.acc = self.strategy.apply(self.acc.take(), value); }

  fn error(self, err: Err) { self.observer.error(err); }

  fn complete(mut self) {
    if let Some(acc) = self.acc.take() {
      self.observer.next(acc);
    }
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn reduce_initial() {
    let mut emitted = 0;
    Local::from_iter([1, 1, 1, 1, 1])
      .reduce_initial(100, |acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(105, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_initial_on_empty() {
    let mut emitted = 0;
    Local::from_iter(std::iter::empty::<i32>())
      .reduce_initial(100, |acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(100, emitted);
  }

  #[rxrust_macro::test]
  fn reduce() {
    let mut emitted = 0;
    Local::from_iter([1, 1, 1, 1, 1])
      .reduce(|acc: i32, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(5, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_multiplication() {
    let mut emitted = 0;
    Local::from_iter([2, 3, 4])
      .reduce(|acc, v| acc * v)
      .subscribe(|v| emitted = v);

    assert_eq!(24, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_on_empty() {
    let mut emitted = 0;
    Local::from_iter(std::iter::empty::<i32>())
      .reduce(|acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(0, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_mixed_types() {
    let mut emitted = 0u32;
    Local::from_iter([1u32, 2u32, 3u32, 4u32])
      .reduce(|acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(10u32, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_initial_mixed_types() {
    let mut emitted = 0;
    Local::from_iter([String::from("foo"), String::from("bar")])
      .reduce_initial(0, |acc, v| acc + v.len())
      .subscribe(|v| emitted = v);

    assert_eq!(6, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_single_item() {
    let mut emitted = 0;
    Local::from_iter([42])
      .reduce(|acc: i32, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(42, emitted);
  }

  #[rxrust_macro::test]
  fn reduce_completes_correctly() {
    let completed = Rc::new(Cell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3])
      .reduce(|acc: i32, v| acc + v)
      .on_complete(move || completed_clone.set(true))
      .subscribe(|_| {});

    assert!(completed.get());
  }

  #[rxrust_macro::test]
  fn reduce_empty_completes_correctly() {
    let completed = Rc::new(Cell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .reduce(|acc: i32, v| acc + v)
      .on_complete(move || completed_clone.set(true))
      .subscribe(|_| {});

    assert!(completed.get());
  }
}
