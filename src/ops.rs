pub mod delay;
pub mod filter;
pub mod filter_map;
pub mod first;
pub mod last;
pub mod map;
pub mod merge;
pub mod observe_on;
pub mod ref_count;
pub mod scan;
pub mod skip;
pub mod skip_last;
pub mod subscribe_on;
pub mod take;
pub mod take_last;
pub mod throttle_time;
pub use filter_map::FilterMap;
pub mod box_it;
pub mod zip;

use last::LastOrOp;
use map::MapOp;
use scan::ScanOp;

pub type CountOp<Source, Item> =
  ReduceOp<Source, fn(usize, Item) -> usize, usize>;
pub type SumOp<Source, Item> = ReduceOp<Source, fn(Item, Item) -> Item, Item>;

// A composition of `scan` followed by `last`
pub type ReduceOp<Source, BinaryOp, OutputItem> =
  LastOrOp<ScanOp<Source, BinaryOp, OutputItem>, OutputItem>;
/// Realised as chained composition of scan->last->map operators.
pub type MinMaxOp<Source, Item> = MapOp<
  LastOrOp<
    ScanOp<Source, fn(Option<Item>, Item) -> Option<Item>, Option<Item>>,
    Option<Item>,
  >,
  fn(Option<Item>) -> Item,
>;

/// Holds intermediate computations of accumulated values for
/// [`Observable@Average`] operator, as nominator and denominator respectively.
pub type Accum<Item> = (Item, usize);

/// Realised as chained composition of scan->last->map operators.
pub type AverageOp<Source, Item> = MapOp<
  LastOrOp<
    ScanOp<Source, fn(Accum<Item>, Item) -> Accum<Item>, Accum<Item>>,
    Accum<Item>,
  >,
  fn(Accum<Item>) -> Item,
>;

#[cfg(test)]
mod test {
  use crate::prelude::*;

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
    m.reduce(|acc: i32, v| acc + v)
      .to_shared()
      .to_shared()
      .subscribe(|_| {});
  }

  use float_cmp::*;

  // -------------------------------------------------------------------
  // testing Max operator
  // -------------------------------------------------------------------

  #[test]
  fn max_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![3., 4., 5., 6., 7.])
      .max()
      .subscribe_all(
        |v| {
          num_emissions += 1;
          emitted = v
        },
        |_| num_errors += 1,
        || num_completions += 1,
      );
    assert!(approx_eq!(f64, 7.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors);
    assert_eq!(1, num_completions);
  }

  #[test]
  fn max_of_floats_negative_values() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![-3., -4., -5., -6., -7.])
      .max()
      .subscribe_all(
        |v| {
          num_emissions += 1;
          emitted = v
        },
        |_| num_errors += 1,
        || num_completions += 1,
      );
    assert!(approx_eq!(f64, -3.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors);
    assert_eq!(1, num_completions);
  }

  #[test]
  fn max_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    observable::of(123.0).max().subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });
    assert!(approx_eq!(f64, 123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[test]
  fn max_on_empty_observable() {
    let mut emitted: Option<f64> = None;
    observable::empty().max().subscribe(|v| emitted = Some(v));
    assert_eq!(None, emitted);
  }

  #[test]

  fn max_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(vec![1., 2.]).max();
    m.to_shared().to_shared().subscribe(|_| {});
  }

  // -------------------------------------------------------------------
  // testing Min operator
  // -------------------------------------------------------------------

  #[test]
  fn min_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![3., 4., 5., 6., 7.])
      .min()
      .subscribe_all(
        |v| {
          num_emissions += 1;
          emitted = v
        },
        |_| num_errors += 1,
        || num_completions += 1,
      );
    assert!(approx_eq!(f64, 3.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors);
    assert_eq!(1, num_completions);
  }

  #[test]
  fn min_of_floats_negative_values() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![-3., -4., -5., -6., -7.])
      .min()
      .subscribe_all(
        |v| {
          num_emissions += 1;
          emitted = v
        },
        |_| num_errors += 1,
        || num_completions += 1,
      );
    assert!(approx_eq!(f64, -7.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors);
    assert_eq!(1, num_completions);
  }

  #[test]
  fn min_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    observable::of(123.0).min().subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });
    assert!(approx_eq!(f64, 123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[test]
  fn min_on_empty_observable() {
    let mut emitted: Option<f64> = None;
    observable::empty().min().subscribe(|v| emitted = Some(v));
    assert_eq!(None, emitted);
  }

  #[test]

  fn min_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(vec![1., 2.]).min();
    m.to_shared().to_shared().subscribe(|_| {});
  }

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
    m.sum().to_shared().to_shared().subscribe(|_| {});
  }

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
    observable::empty::<i32>()
      .count()
      .subscribe(|v| emitted = v);
    assert_eq!(0, emitted);
  }

  #[test]
  fn count_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).count();
    m.to_shared().to_shared().subscribe(|_| {});
  }

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
          emitted = v
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
      emitted = v
    });
    assert!(approx_eq!(f64, 123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[test]
  fn average_on_empty_observable() {
    let mut emitted: Option<f64> = None;
    observable::empty()
      .average()
      .subscribe(|v| emitted = Some(v));
    assert_eq!(None, emitted);
  }

  #[test]

  fn average_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(vec![1., 2.]).average();
    m.to_shared().to_shared().subscribe(|_| {});
  }
}
