use crate::prelude::*;
use ops::last::{Last, LastOrOp};
use ops::map::{Map, MapOp};
use ops::scan::{Scan, ScanOp};
use std::ops::Add;
use std::ops::Mul;

/// Holds intermediate computations of accumulated values for [`Average`]
/// operator, as nominator and denominator respectively.
type Accum<Item> = (Item, usize);

/// Computing an average by multiplying accumulated nominator by a reciprocal
/// of accumulated denominator. In this way some generic types that support
/// linear scaling over floats values could be averaged (e.g. vectors)
fn average_floats<T>(acc: &Accum<T>) -> T
where
  T: Default + Copy + Send + Mul<f64, Output = T>,
{
  // Note: we will never be dividing by zero here, as
  // the acc.1 will be always >= 1.
  // It would have be zero if we've would have received an element
  // when the source observable is empty but beacuse of how
  // `scan` works, we will transparently not receive anything in
  // such case.
  acc.0 * (1.0 / (acc.1 as f64))
}

fn accumulate_item<T>(acc: &Accum<T>, v: &T) -> Accum<T>
where
  T: Copy + Add<T, Output = T>,
{
  let newacc = acc.0 + *v;
  let newcount = acc.1 + 1;
  (newacc, newcount)
}

/// Realised as chained composition of scan->last->map operators.
pub type AverageOp<Source, Item> = MapOp<
  LastOrOp<
    ScanOp<Source, fn(&Accum<Item>, &Item) -> Accum<Item>, Item, Accum<Item>>,
    Accum<Item>,
  >,
  fn(&Accum<Item>) -> Item,
  Accum<Item>,
>;

pub trait Average<Item>
where
  Self: Sized,
{
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
  /// use rxrust::ops::Average;
  ///
  /// observable::from_iter(vec![3., 4., 5., 6., 7.])
  ///   .average()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  ///
  fn average(self) -> AverageOp<Self, Item>;
}

/// Implementation for types that scale with multiplying by a f64 value
/// (e.g f64 numbers itselfs, complex numbers, vectors, matrices).
impl<O, Item> Average<Item> for O
where
  Self: Sized,
  Item:
    Copy + Send + Default + Add<Item, Output = Item> + Mul<f64, Output = Item>,
{
  fn average(self) -> AverageOp<Self, Item> {
    // our starting point
    let start = (Item::default(), 0);

    let acc = accumulate_item as fn(&Accum<Item>, &Item) -> Accum<Item>;
    let avg = average_floats as fn(&Accum<Item>) -> Item;

    self.scan_initial(start, acc).last().map(avg)
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Average, prelude::*};
  use float_cmp::*;

  #[test]
  fn average_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![3., 4., 5., 6., 7.])
      .average()
      .subscribe_all(
        |v| {
          num_emissions += 1;
          emitted = *v
        },
        |_| num_errors += 1,
        || num_completions += 1,
      );
    assert!(approx_eq!(f64, 5.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors);
    assert_eq!(1, num_completions);
  }

  // TODO: this test ideally should be passing, but for now ints have no
  // default operation of multiplying by f64, so leaving for later
  // #[test]
  // fn average_of_ints() {
  //   let mut emitted = 0.0;
  //   let mut num_emissions = 0;
  //   let mut num_errors = 0;
  //   let mut num_completions = 0;
  //   observable::from_iter(vec![3, 4, 5, 6, 7])
  //     .average()
  //     .subscribe_all(
  //       |v| {
  //         num_emissions += 1;
  //         emitted = *v
  //       },
  //       |_| num_errors += 1,
  //       || num_completions += 1,
  //     );
  //   // TODO: never compare floats directly
  //   assert_eq!(5.0, emitted);
  //   assert_eq!(1, num_emissions);
  //   assert_eq!(0, num_errors);
  //   assert_eq!(1, num_completions);
  // }

  #[test]
  fn average_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    observable::of(123.0).average().subscribe(|v| {
      num_emissions += 1;
      emitted = *v
    });
    assert!(approx_eq!(f64, 123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[test]
  fn average_on_empty_observable() {
    let mut emitted: Option<f64> = None;
    observable::empty()
      .average()
      .subscribe(|v| emitted = Some(*v));
    assert_eq!(None, emitted);
  }

  #[test]

  fn average_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(vec![1., 2.]).average();
    m.fork().to_shared().fork().to_shared().subscribe(|_| {});
  }
}
