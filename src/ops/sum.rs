use crate::prelude::*;
use ops::reduce::{Reduce, ReduceOp};
use std::ops::Add;

pub type SumOp<Source, Item> =
  ReduceOp<Source, fn(Item, Item) -> Item, Item, Item>;

pub trait Sum<Item> {
  /// Calculates the sum of numbers emitted by an source observable and emits
  /// this sum when source completes.
  ///
  /// Emits zero when source completed as an and empty sequence.
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Sum;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .sum()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  ///
  fn sum(self) -> SumOp<Self, Item>
  where
    Self: Sized,
    Item: Copy + Default + Add<Item, Output = Item>,
  {
    self.reduce(|acc, v| acc + v)
  }
}

impl<O, Item> Sum<Item> for O {}

#[cfg(test)]
mod test {
  use crate::{ops::Sum, prelude::*};

  #[test]
  fn sum() {
    let mut emitted = 0;
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .sum()
      .subscribe(|v| emitted = v);
    assert_eq!(5, emitted);
  }

  #[test]
  fn sum_on_single_item() {
    let mut emitted = 0;
    observable::of(123).sum().subscribe(|v| emitted = v);
    assert_eq!(123, emitted);
  }

  #[test]
  fn sum_on_empty_observable() {
    let mut emitted = 0;
    observable::empty().sum().subscribe(|v| emitted = v);
    assert_eq!(0, emitted);
  }

  #[test]
  fn sum_on_mixed_sign_values() {
    let mut emitted = 0;
    observable::from_iter(vec![1, -1, 1, -1, -1])
      .sum()
      .subscribe(|v| emitted = v);
    assert_eq!(-1, emitted);
  }

  #[test]
  fn sum_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).sum();
    m.fork()
      .sum()
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
