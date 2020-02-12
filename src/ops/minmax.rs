use crate::prelude::*;
use ops::last::{Last, LastOrOp};
use ops::map::{Map, MapOp};
use ops::scan::{Scan, ScanOp};
use std::cmp::PartialOrd;
use std::option::Option;

/// Realised as chained composition of scan->last->map operators.
pub type MinMaxOp<Source, Item> = MapOp<
  LastOrOp<
    ScanOp<Source, fn(Option<Item>, Item) -> Option<Item>, Option<Item>>,
    Option<Item>,
  >,
  fn(Option<Item>) -> Item,
>;

fn get_greater<Item>(i: Option<Item>, v: Item) -> Option<Item>
where
  Item: Copy + PartialOrd<Item>,
{
  // using universal function call because of name collision with our `map`
  // being declared in the scope.
  std::option::Option::map(i, |vv| if vv < v { v } else { vv }).or(Some(v))
}
fn get_lesser<Item>(i: Option<Item>, v: Item) -> Option<Item>
where
  Item: Copy + PartialOrd<Item>,
{
  std::option::Option::map(i, |vv| if vv > v { v } else { vv }).or(Some(v))
}

/// Defines `min` and `max` operators for observables.
pub trait MinMax<Item>
where
  Self: Sized,
{
  /// Emits the item from the source observable that had the maximum value.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::MinMax;
  ///
  /// observable::from_iter(vec![3., 4., 7., 5., 6.])
  ///   .max()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 7
  /// ```
  ///
  fn max(self) -> MinMaxOp<Self, Item>
  where
    Self: Sized,
    Item: Copy + Send + PartialOrd<Item>,
  {
    let get_greater_func =
      get_greater as fn(Option<Item>, Item) -> Option<Item>;

    self
      .scan_initial(None, get_greater_func)
      .last()
      // we can safely unwrap, because we will ever get this item
      // once a max value exists and is there.
      .map(|v| v.unwrap())
  }

  /// Emits the item from the source observable that had the minimum value.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::MinMax;
  ///
  /// observable::from_iter(vec![3., 4., 7., 5., 6.])
  ///   .min()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 3
  /// ```
  ///
  fn min(self) -> MinMaxOp<Self, Item>
  where
    Self: Sized,
    Item: Copy + Send + PartialOrd<Item>,
  {
    let get_lesser_func = get_lesser as fn(Option<Item>, Item) -> Option<Item>;

    self
      .scan_initial(None, get_lesser_func)
      .last()
      // we can safely unwrap, because we will ever get this item
      // once a max value exists and is there.
      .map(|v| v.unwrap())
  }
}

impl<O, Item> MinMax<Item> for O {}

#[cfg(test)]
mod test {
  use crate::{ops::MinMax, prelude::*};
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
    m.clone().to_shared().clone().to_shared().subscribe(|_| {});
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
    m.clone().to_shared().clone().to_shared().subscribe(|_| {});
  }
}
