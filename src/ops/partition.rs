//! Partition operator implementation
//!
//! Splits a source into the items that satisfy a predicate and the rest.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Partition operator: One half of a `partition` pair
///
/// Forwards items whose predicate result equals `keep`. `partition` builds
/// two of these over a cloned source: one with `keep = true`, one with
/// `keep = false`. Each half subscribes to the source on its own, as in
/// RxJS; use `share()` first if the source must be subscribed once.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let (evens, odds) = Local::from_iter(vec![1, 2, 3, 4]).partition(|v| v % 2 == 0);
/// let mut seen_even = Vec::new();
/// let mut seen_odd = Vec::new();
/// evens.subscribe(|v| seen_even.push(v));
/// odds.subscribe(|v| seen_odd.push(v));
/// assert_eq!(seen_even, vec![2, 4]);
/// assert_eq!(seen_odd, vec![1, 3]);
/// ```
#[derive(Clone)]
pub struct Partition<S, F> {
  pub source: S,
  pub predicate: F,
  pub keep: bool,
}

impl<S: ObservableType, F> ObservableType for Partition<S, F> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that keeps items matching the wanted predicate result
pub struct PartitionObserver<O, F> {
  observer: O,
  predicate: F,
  keep: bool,
}

impl<O, F, Item, Err> Observer<Item, Err> for PartitionObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, value: Item) {
    if (self.predicate)(&value) == self.keep {
      self.observer.next(value);
    }
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, C> CoreObservable<C> for Partition<S, F>
where
  C: Context,
  S: CoreObservable<C::With<PartitionObserver<C::Inner, F>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Partition { source, predicate, keep } = self;
    let wrapped = context.transform(|observer| PartitionObserver { observer, predicate, keep });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_partition_splits_items() {
    let evens = Rc::new(RefCell::new(Vec::new()));
    let odds = Rc::new(RefCell::new(Vec::new()));
    let evens_c = evens.clone();
    let odds_c = odds.clone();

    let (matching, rest) = Local::from_iter(vec![1, 2, 3, 4, 5]).partition(|v| v % 2 == 0);
    matching.subscribe(move |v| evens_c.borrow_mut().push(v));
    rest.subscribe(move |v| odds_c.borrow_mut().push(v));

    assert_eq!(*evens.borrow(), vec![2, 4]);
    assert_eq!(*odds.borrow(), vec![1, 3, 5]);
  }

  #[rxrust_macro::test]
  fn test_partition_each_half_subscribes_independently() {
    let mut source = Local::subject::<i32, Infallible>();
    let (matching, rest) = source.clone().partition(|v| *v > 0);
    let _a = matching.subscribe(|_| {});
    let _b = rest.subscribe(|_| {});
    assert_eq!(source.inner.subscriber_count(), 2);
    source.next(1);
  }

  #[rxrust_macro::test]
  fn test_partition_error_reaches_both() {
    let errors = Rc::new(RefCell::new(0));
    let e1 = errors.clone();
    let e2 = errors.clone();

    let (matching, rest) = Local::throw_err("boom".to_string())
      .map(|_| 0)
      .partition(|v| *v > 0);
    matching
      .on_error(move |_| *e1.borrow_mut() += 1)
      .subscribe(|_| {});
    rest
      .on_error(move |_| *e2.borrow_mut() += 1)
      .subscribe(|_| {});

    assert_eq!(*errors.borrow(), 2);
  }
}
