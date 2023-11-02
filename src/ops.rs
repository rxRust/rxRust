pub mod box_it;
pub mod buffer;
pub mod collect;
pub mod combine_latest;
pub mod complete_status;
pub mod contains;
pub mod debounce;
pub mod default_if_empty;
pub mod delay;
pub mod distinct;
pub mod filter;
pub mod filter_map;
pub mod finalize;
pub mod future;
pub mod group_by;
pub mod last;
pub mod map;
pub mod map_to;
pub mod merge;
pub mod merge_all;
pub mod observe_on;
pub mod on_complete;
pub mod on_error;
pub mod on_error_map;
pub mod pairwise;
pub mod ref_count;
pub mod sample;
pub mod scan;
pub mod skip;
pub mod skip_last;
pub mod skip_until;
pub mod skip_while;
pub mod start_with;
pub mod stream;
pub mod subscribe_on;
pub mod take;
pub mod take_last;
pub mod take_until;
pub mod take_while;
pub mod tap;
pub mod throttle;
pub mod timestamp;
pub mod with_latest_from;
pub mod zip;

use default_if_empty::DefaultIfEmptyOp;
use last::LastOp;
use map::MapOp;
use scan::ScanOp;

use self::merge_all::{MergeAllOp, MergeAllOpThreads};

pub type CountOp<Source, Item> =
  ReduceOp<Source, fn(usize, Item) -> usize, usize, Item>;
pub type SumOp<Source, Item> =
  ReduceOp<Source, fn(Item, Item) -> Item, Item, Item>;

// A composition of `scan` followed by `last`
pub type ReduceOp<Source, BinaryOp, OutputItem, InputItem> = DefaultIfEmptyOp<
  LastOp<ScanOp<Source, BinaryOp, OutputItem, InputItem>, OutputItem>,
  OutputItem,
>;
/// Realised as chained composition of scan->last->map operators.
pub type MinMaxOp<Source, Item> = MapOp<
  LastOp<
    ScanOp<Source, fn(Option<Item>, Item) -> Option<Item>, Option<Item>, Item>,
    Option<Item>,
  >,
  fn(Option<Item>) -> Item,
  Option<Item>,
>;

/// Holds intermediate computations of accumulated values for
/// [`Observable@Average`] operator, as nominator and denominator respectively.
pub type Accum<Item> = (Item, usize);

/// Realised as chained composition of scan->last->map operators.
pub type AverageOp<Source, Item> = MapOp<
  LastOp<
    ScanOp<Source, fn(Accum<Item>, Item) -> Accum<Item>, Accum<Item>, Item>,
    Accum<Item>,
  >,
  fn(Accum<Item>) -> Item,
  Accum<Item>,
>;

/// Returns an Observable that emits items based on applying a function that you
/// supply to each item emitted by the source Observable, where that function
/// returns an Observable, and then merging those resulting Observables and
/// emitting the results of this merger.
pub type FlatMapOp<'a, Source, OutputItem, F, Input> =
  MergeAllOp<'a, MapOp<Source, F, Input>, OutputItem>;
pub type FlatMapOpThreads<Source, OutputItem, F, Input> =
  MergeAllOpThreads<MapOp<Source, F, Input>, OutputItem>;

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
      .on_error(|_| num_errors += 1)
      .on_complete(|| num_completions += 1)
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });
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
      .on_error(|_| num_errors += 1)
      .on_complete(|| num_completions += 1)
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });
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
      .on_error(|_| num_errors += 1)
      .on_complete(|| num_completions += 1)
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });
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
      .on_error(|_| num_errors += 1)
      .on_complete(|| num_completions += 1)
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });
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
    ObservableExt::<i32, _>::count(observable::empty())
      .subscribe(|v| emitted = v);
    assert_eq!(0, emitted);
  }

  #[test]
  fn average_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let mut num_errors = 0;
    let mut num_completions = 0;
    observable::from_iter(vec![3., 4., 5., 6., 7.])
      .average()
      .on_error(|_| num_errors += 1)
      .on_complete(|| num_completions += 1)
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });
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

  // -------------------------------------------------------------------
  // testing ConcatMap operator
  // -------------------------------------------------------------------

  #[test]
  fn concat_map_identity() {
    let return_fn = observable::of;
    let f = |x| observable::of(x + 1);
    let m = observable::of(0_i32);

    // left identity
    let partial_left = |x| return_fn(x).concat_map(f);
    let comp_left = m.clone().concat_map(partial_left);

    // right identity
    let partial_right = |x| f(x).concat_map(return_fn);
    let comp_right = m.concat_map(partial_right);

    let mut left: Option<i32> = None;
    let mut right: Option<i32> = None;

    comp_left.subscribe(|a| left = Some(a));
    comp_right.subscribe(|b| right = Some(b));

    assert_eq!(left, right);
  }

  #[test]
  fn concat_map_associative() {
    let f = |i: i32| observable::of(i + 1);
    let g = |i: i32| observable::of(i + 2);
    let h = |i: i32| observable::of(i + 3);
    let m = observable::of(0_i32);

    // left association
    let partial_left = |x| {
      let partial = f(x).concat_map(g);
      partial.concat_map(h)
    };
    let comp_left = m.clone().concat_map(partial_left);

    // right association
    let partial_right = |x| {
      let partial = |y| g(y).concat_map(h);
      f(x).concat_map(partial)
    };
    let comp_right = m.concat_map(partial_right);

    let mut left: Option<i32> = None;
    let mut right: Option<i32> = None;

    comp_left.subscribe(|a| left = Some(a));
    comp_right.subscribe(|b| right = Some(b));

    assert_eq!(left, right);
  }

  // -------------------------------------------------------------------
  // testing FlatMap operator
  // -------------------------------------------------------------------

  #[test]
  fn flat_map_identity() {
    let return_fn = observable::of;
    let f = |x| observable::of(x + 1);
    let m = observable::of(0_i32);

    // left identity
    let partial_left = |x| return_fn(x).flat_map(f);
    let comp_left = m.clone().flat_map(partial_left);

    // right identity
    let partial_right = |x| f(x).flat_map(return_fn);
    let comp_right = m.flat_map(partial_right);

    let mut left: Option<i32> = None;
    let mut right: Option<i32> = None;

    comp_left.subscribe(|a| left = Some(a));
    comp_right.subscribe(|b| right = Some(b));

    assert_eq!(left, right);
  }

  #[test]
  fn flat_map_associative() {
    let f = |i: i32| observable::of(i + 1);
    let g = |i: i32| observable::of(i + 2);
    let h = |i: i32| observable::of(i + 3);
    let m = observable::of(0_i32);

    // left association
    let partial_left = |x| {
      let partial = f(x).flat_map(g);
      partial.flat_map(h)
    };
    let comp_left = m.clone().flat_map(partial_left);

    // right association
    let partial_right = |x| {
      let partial = |y| g(y).flat_map(h);
      f(x).flat_map(partial)
    };
    let comp_right = m.flat_map(partial_right);

    let mut left: Option<i32> = None;
    let mut right: Option<i32> = None;

    comp_left.subscribe(|a| left = Some(a));
    comp_right.subscribe(|b| right = Some(b));

    assert_eq!(left, right);
  }
}
