#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  fn approx_eq(expected: f64, actual: f64) -> bool { (expected - actual).abs() <= 1e-9 }

  use crate::prelude::*;

  // -------------------------------------------------------------------
  // testing Max operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn max_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let num_errors = Rc::new(Cell::new(0));
    let num_completions = Rc::new(Cell::new(0));
    let num_errors_clone = num_errors.clone();
    let num_completions_clone = num_completions.clone();

    Local::from_iter([3., 4., 5., 6., 7.])
      .max()
      .on_error(move |_| num_errors_clone.set(num_errors_clone.get() + 1))
      .on_complete(move || num_completions_clone.set(num_completions_clone.get() + 1))
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });

    assert!(approx_eq(7.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors.get());
    assert_eq!(1, num_completions.get());
  }

  #[rxrust_macro::test]
  fn max_of_floats_negative_values() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let num_errors = Rc::new(Cell::new(0));
    let num_completions = Rc::new(Cell::new(0));
    let num_errors_clone = num_errors.clone();
    let num_completions_clone = num_completions.clone();

    Local::from_iter([-3., -4., -5., -6., -7.])
      .max()
      .on_error(move |_| num_errors_clone.set(num_errors_clone.get() + 1))
      .on_complete(move || num_completions_clone.set(num_completions_clone.get() + 1))
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });

    assert!(approx_eq(-3.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors.get());
    assert_eq!(1, num_completions.get());
  }

  #[rxrust_macro::test]
  fn max_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;

    Local::of(123.0).max().subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });

    assert!(approx_eq(123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[rxrust_macro::test]
  fn max_on_empty_observable() {
    let mut emitted: Option<f64> = None;

    Local::from_iter(std::iter::empty::<f64>())
      .max()
      .subscribe(|v| emitted = Some(v));

    assert_eq!(None, emitted);
  }

  // -------------------------------------------------------------------
  // testing Min operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn min_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let num_errors = Rc::new(Cell::new(0));
    let num_completions = Rc::new(Cell::new(0));
    let num_errors_clone = num_errors.clone();
    let num_completions_clone = num_completions.clone();

    Local::from_iter([3., 4., 5., 6., 7.])
      .min()
      .on_error(move |_| num_errors_clone.set(num_errors_clone.get() + 1))
      .on_complete(move || num_completions_clone.set(num_completions_clone.get() + 1))
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });

    assert!(approx_eq(3.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors.get());
    assert_eq!(1, num_completions.get());
  }

  #[rxrust_macro::test]
  fn min_of_floats_negative_values() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let num_errors = Rc::new(Cell::new(0));
    let num_completions = Rc::new(Cell::new(0));
    let num_errors_clone = num_errors.clone();
    let num_completions_clone = num_completions.clone();

    Local::from_iter([-3., -4., -5., -6., -7.])
      .min()
      .on_error(move |_| num_errors_clone.set(num_errors_clone.get() + 1))
      .on_complete(move || num_completions_clone.set(num_completions_clone.get() + 1))
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });

    assert!(approx_eq(-7.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors.get());
    assert_eq!(1, num_completions.get());
  }

  #[rxrust_macro::test]
  fn min_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;

    Local::of(123.0).min().subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });

    assert!(approx_eq(123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[rxrust_macro::test]
  fn min_on_empty_observable() {
    let mut emitted: Option<f64> = None;

    Local::from_iter(std::iter::empty::<f64>())
      .min()
      .subscribe(|v| emitted = Some(v));

    assert_eq!(None, emitted);
  }

  // -------------------------------------------------------------------
  // testing Sum operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn sum() {
    let mut emitted = 0;

    Local::from_iter([1, 1, 1, 1, 1])
      .sum()
      .subscribe(|v| emitted = v);

    assert_eq!(5, emitted);
  }

  #[rxrust_macro::test]
  fn sum_on_single_item() {
    let mut emitted = 0;
    Local::of(123).sum().subscribe(|v| emitted = v);
    assert_eq!(123, emitted);
  }

  #[rxrust_macro::test]
  fn sum_on_empty_observable() {
    let mut emitted = 0;

    Local::from_iter(std::iter::empty::<i32>())
      .sum()
      .subscribe(|v| emitted = v);

    assert_eq!(0, emitted);
  }

  #[rxrust_macro::test]
  fn sum_on_mixed_sign_values() {
    let mut emitted = 0;

    Local::from_iter([1, -1, 1, -1, -1])
      .sum()
      .subscribe(|v| emitted = v);

    assert_eq!(-1, emitted);
  }

  // -------------------------------------------------------------------
  // testing Count operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn count() {
    let mut emitted = 0;

    Local::from_iter(['1', '7', '3', '0', '4'])
      .count()
      .subscribe(|v| emitted = v);

    assert_eq!(5, emitted);
  }

  #[rxrust_macro::test]
  fn count_on_empty_observable() {
    let mut emitted = 0;

    Local::from_iter(std::iter::empty::<i32>())
      .count()
      .subscribe(|v| emitted = v);

    assert_eq!(0, emitted);
  }

  // -------------------------------------------------------------------
  // testing Average operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn average_of_floats() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;
    let num_errors = Rc::new(Cell::new(0));
    let num_completions = Rc::new(Cell::new(0));
    let num_errors_clone = num_errors.clone();
    let num_completions_clone = num_completions.clone();

    Local::from_iter([3., 4., 5., 6., 7.])
      .average()
      .on_error(move |_| num_errors_clone.set(num_errors_clone.get() + 1))
      .on_complete(move || num_completions_clone.set(num_completions_clone.get() + 1))
      .subscribe(|v| {
        num_emissions += 1;
        emitted = v
      });

    assert!(approx_eq(5.0, emitted));
    assert_eq!(1, num_emissions);
    assert_eq!(0, num_errors.get());
    assert_eq!(1, num_completions.get());
  }

  #[rxrust_macro::test]
  fn average_on_single_float_item() {
    let mut emitted = 0.0;
    let mut num_emissions = 0;

    Local::of(123.0).average().subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });

    assert!(approx_eq(123.0, emitted));
    assert_eq!(1, num_emissions);
  }

  #[rxrust_macro::test]
  fn average_on_empty_observable() {
    let mut emitted: Option<f64> = None;

    Local::from_iter(std::iter::empty::<f64>())
      .average()
      .subscribe(|v| emitted = Some(v));

    assert_eq!(None, emitted);
  }

  // -------------------------------------------------------------------
  // testing ConcatMap operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn concat_map_flattens_sequentially() {
    let mut out = Vec::new();

    Local::from_iter([1_i32, 2_i32, 3_i32])
      .concat_map(|x| Local::from_iter([x, x + 10]))
      .subscribe(|v| out.push(v));

    assert_eq!(out, vec![1, 11, 2, 12, 3, 13]);
  }

  // -------------------------------------------------------------------
  // testing FlatMap operator
  // -------------------------------------------------------------------

  #[rxrust_macro::test]
  fn flat_map_merges_inner_observables() {
    let mut out = Vec::new();

    Local::from_iter([1_i32, 2_i32, 3_i32])
      .flat_map(|x| Local::from_iter([x, x + 10]))
      .subscribe(|v| out.push(v));

    // With synchronous inner observables, MergeAll subscribes immediately.
    assert_eq!(out, vec![1, 11, 2, 12, 3, 13]);
  }
}
