use crate::prelude::*;
use ops::last::{Last, LastOrOp};
use ops::scan::{Scan, ScanOp};

// A composition of `scan` followed by `last`
pub type ReduceOp<Source, BinaryOp, InputItem, OutputItem> =
  LastOrOp<ScanOp<Source, BinaryOp, InputItem, OutputItem>, OutputItem>;

/// The [`Reduce`] operator applies a function to the first item emitted by the
/// source observable and then feeds the result of the function back into the
/// function along with the second item emitted by the source observable,
/// continuing this process until the source observable emits its final item
/// and completes, whereupon the observable returned from [`Reduce`] emits the
/// final value returned from the function.
pub trait Reduce<OutputItem> {
  /// Apply a function to each item emitted by an observable, sequentially,
  /// and emit the final value, after source observable completes.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Arguments
  ///
  /// * `initial` - An initial value to start the successive reduction from.
  /// * `binary_op` - A closure acting as a binary (folding) operator.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Reduce;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .reduce_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 105
  /// ```
  ///
  fn reduce_initial<InputItem, BinaryOp>(
    self,
    initial: OutputItem,
    binary_op: BinaryOp,
  ) -> ReduceOp<Self, BinaryOp, InputItem, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(OutputItem, InputItem) -> OutputItem,
    OutputItem: Clone,
  {
    // realised as a composition of `scan`, and `last`
    self
      .scan_initial(initial.clone(), binary_op)
      .last_or(initial)
  }

  /// Works like [`reduce_initial`] but starts with a value defined by a
  /// [`Default`] trait for the first argument `f` operator operates on.
  ///
  /// # Arguments
  ///
  /// * `binary_op` - A closure acting as a binary operator.
  ///
  fn reduce<InputItem, BinaryOp>(
    self,
    binary_op: BinaryOp,
  ) -> LastOrOp<ScanOp<Self, BinaryOp, InputItem, OutputItem>, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(OutputItem, InputItem) -> OutputItem,
    OutputItem: Default + Clone,
  {
    self.reduce_initial(OutputItem::default(), binary_op)
  }
}

impl<O, OutputItem> Reduce<OutputItem> for O {}

#[cfg(test)]
mod test {
  use crate::{ops::Reduce, prelude::*};

  #[test]
  fn reduce_initial() {
    let mut emitted = 0;
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .reduce_initial(100, |acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(105, emitted);
  }

  #[test]
  fn reduce_initial_on_empty_observable() {
    let mut emitted = 0;
    observable::empty()
      .reduce_initial(100, |acc, v: i32| acc + v)
      .subscribe(|v| emitted = v);

    // expected to emit the initial value
    assert_eq!(100, emitted);
  }
  #[test]
  fn reduce() {
    let mut emitted = 0;
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .reduce(|acc, v| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(5, emitted);
  }

  #[test]
  fn reduce_on_empty_observable() {
    let mut emitted = 0;
    observable::empty()
      .reduce(|acc, v: i32| acc + v)
      .subscribe(|v| emitted = v);

    assert_eq!(0, emitted);
  }

  #[test]
  fn reduce_mixed_types() {
    // we're using mixed numeric types here to perform transform
    let mut emitted = 0u32;
    observable::from_iter(vec![1i32, 2i32, 3i32, 4i32])
      .reduce(|acc, v: i32| acc + (v as u32))
      .subscribe(|v| emitted = v);

    assert_eq!(10u32, emitted);
  }
  #[test]
  fn reduce_for_counting_total_length() {
    let mut emitted = 0;
    observable::from_iter(vec![String::from("foo"), String::from("bar")])
      .reduce(|acc, v: String| acc + v.len())
      .subscribe(|v| emitted = v);

    assert_eq!(6, emitted);
  }

  #[test]
  fn reduce_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).reduce(|acc: i32, v| acc + v);
    m.fork()
      .reduce(|acc: i32, v| acc + v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
