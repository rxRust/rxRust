use crate::prelude::*;
use ops::reduce::{Reduce, ReduceOp};

pub type CountOp<Source, Item> =
  ReduceOp<Source, fn(usize, Item) -> usize, usize>;

pub trait Count {
  /// Emits the number of items emitted by a source observable when this source
  /// completes.
  ///
  /// The output type of this operator is fixed to [`usize`].
  ///
  /// Emits zero when source completed as an and empty sequence.
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Count;
  ///
  /// observable::from_iter(vec!['1', '7', '3', '0', '4'])
  ///   .count()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  fn count<Item>(self) -> CountOp<Self, Item>
  where
    Self: Sized,
  {
    self.reduce(|acc, _v| acc + 1)
  }
}

impl<O> Count for O {}

#[cfg(test)]
mod test {
  use crate::{ops::Count, prelude::*};

  #[test]
  fn count() {
    let mut emitted = 0;
    observable::from_iter(vec!['1', '7', '3', '0', '4'])
      .count()
      .subscribe(|v| emitted = v);
    assert_eq!(5, emitted);
  }

  #[test]
  fn count_on_empty_observable() {
    let mut emitted = 0;
    observable::empty()
      .count::<i32>()
      .subscribe(|v| emitted = v);
    assert_eq!(0, emitted);
  }

  #[test]
  fn count_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).count();
    m.to_shared().to_shared().subscribe(|_| {});
  }
}
