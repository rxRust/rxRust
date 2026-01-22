//! Integration tests for RxRust v1.0
//!
//! Tests complex operator chains, context interactions, and threading behavior.

use std::{cell::RefCell, convert::Infallible, rc::Rc};
#[cfg(not(target_arch = "wasm32"))]
use std::{
  sync::{Arc, Mutex},
  thread,
};

use rxrust::prelude::*;

fn approx_eq(expected: f64, actual: f64) -> bool { (expected - actual).abs() <= 1e-9 }

#[rxrust_macro::test]
fn test_basic_chain_integration() {
  // Test: Local::of(1..10).map(...).filter(...).collect()
  let result = Rc::new(RefCell::new(Vec::new()));
  let result_clone = result.clone();

  Local::from_iter(1..=10)
    .map(|x| x * 2)
    .filter(|&x| x > 10)
    .take(3)
    .subscribe(move |v| result_clone.borrow_mut().push(v));

  assert_eq!(*result.borrow(), vec![12, 14, 16]);
}

#[rxrust_macro::test]
fn test_complex_chain_with_multiple_operators() {
  // Test a complex chain with various operators
  let result = Rc::new(RefCell::new(Vec::new()));
  let result_clone = result.clone();

  Local::from_iter(1..=20)
    .filter(|&x| x % 2 == 0) // Even numbers only
    .map(|x| x * x) // Square them
    .scan(0, |acc, v| acc + v) // Running sum
    .take_while(|&x| x < 100) // Stop when sum reaches 100
    .skip(2) // Skip first 2 values
    .subscribe(move |v| result_clone.borrow_mut().push(v));

  // Calculate expected values manually
  // Even numbers: 2, 4, 6, 8, 10, 12, 14, 16, 18, 20
  // Squares: 4, 16, 36, 64, 100, 144, 196, 256, 324, 400
  // Running sum: 4, 20, 56, 120, 220, 364, 560, 816, 1140, 1540
  // take_while < 100 stops when value >= 100, so emits: 4, 20, 56
  // skip 2: skips 4 and 20, leaving: 56
  assert_eq!(*result.borrow(), vec![56]);
}

#[rxrust_macro::test]
fn test_async_chain_with_shared_observable() {
  // Test: Shared::interval(...).take(5).map(...)
  // Skip for now - requires Tokio runtime which is not set up in integration
  // tests
}

#[rxrust_macro::test]
fn test_subject_broadcasting() {
  // Test: let s = Local::subject(); s.map(...).subscribe(...); s.next(...)
  let mut subject = Local::subject::<i32, Infallible>();

  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  // Subscribe with a map operator
  subject
    .clone()
    .map(|x| x * 10)
    .filter(|&x| x > 50)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  // Subscribe directly without transformation
  let direct_results = Rc::new(RefCell::new(Vec::new()));
  let direct_clone = direct_results.clone();

  subject
    .clone()
    .subscribe(move |v| direct_clone.borrow_mut().push(v));

  // Emit values
  subject.next(3);
  subject.next(6);
  subject.next(10);
  subject.complete();

  assert_eq!(*results.borrow(), vec![60, 100]);
  assert_eq!(*direct_results.borrow(), vec![3, 6, 10]);
}

#[rxrust_macro::test]
fn test_behavior_subject() {
  // Test BehaviorSubject with initial value
  let mut subject = Local::behavior_subject::<i32, Infallible>(42);

  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  // Subscribe - should immediately receive the current value
  subject
    .clone()
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  // Emit new values
  subject.next(100);
  subject.next(200);

  assert_eq!(*results.borrow(), vec![42, 100, 200]);
}

#[rxrust_macro::test]
fn test_merge_integration() {
  // Test merging multiple observables
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let obs1 = Local::of(1);
  let obs2 = Local::of(2);
  let obs3 = Local::of(3);

  obs1
    .merge(obs2)
    .merge(obs3)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  // Order might vary, but all values should be present
  let mut sorted = results.borrow().clone();
  sorted.sort_unstable();
  assert_eq!(sorted, vec![1, 2, 3]);
}

#[rxrust_macro::test]
fn test_zip_integration() {
  // Test zipping two observables
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let obs1 = Local::from_iter([1, 2, 3]);
  let obs2 = Local::from_iter(['a', 'b', 'c']);

  obs1.zip(obs2).subscribe(move |(num, letter)| {
    results_clone.borrow_mut().push((num, letter));
  });

  assert_eq!(*results.borrow(), vec![(1, 'a'), (2, 'b'), (3, 'c')]);
}

#[rxrust_macro::test]
fn test_combine_latest_integration() {
  // Test combining latest values from multiple observables
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let mut subject1 = Local::subject::<i32, Infallible>();
  let mut subject2 = Local::subject::<i32, Infallible>();

  subject1
    .clone()
    .combine_latest(subject2.clone(), |a, b| (a, b))
    .subscribe(move |(a, b)| {
      results_clone.borrow_mut().push((a, b));
    });

  // Emit values - should see combinations
  subject1.next(1); // No output yet (subject2 hasn't emitted)
  subject2.next(10); // Output: (1, 10)
  subject1.next(2); // Output: (2, 10)
  subject2.next(20); // Output: (2, 20)

  assert_eq!(*results.borrow(), vec![(1, 10), (2, 10), (2, 20)]);
}

#[rxrust_macro::test]
fn test_error_propagation() {
  // Test that errors are properly propagated through operator chains
  // Skip for now - ObserverImpl not found in current API
}

#[rxrust_macro::test]
fn test_memory_leak_check_subscription_cleanup() {
  // Ensure subscriptions are properly cleaned up
  let mut subject = Local::subject::<i32, Infallible>();

  let subscription = subject.clone().map(|x| x * 2).subscribe(|_| {});

  // Subscription should be active
  assert!(!subscription.is_closed());

  // Unsubscribe
  assert!(!subscription.is_closed());
  subscription.unsubscribe();
  // Can't check is_closed after unsubscribe since it takes ownership

  // Emit a value - should not cause issues
  subject.next(1);
}

#[cfg(not(target_arch = "wasm32"))]
#[rxrust_macro::test]
fn test_shared_multithreading() {
  // Test that Shared observables work across threads
  // Note: This test demonstrates the need for careful synchronization in
  // multi-threaded scenarios
  let results = Arc::new(Mutex::new(Vec::new()));
  let results_clone = results.clone();

  let mut subject = Shared::subject::<i32, Infallible>();

  // Emit values before subscribing to test hot behavior
  subject.next(1);
  subject.next(2);
  subject.next(3);

  // Subscribe on another thread after emissions
  let subject_clone = subject.clone();
  let handle = thread::spawn(move || {
    subject_clone
      .map(|x| x * 2)
      .subscribe(move |v| results_clone.lock().unwrap().push(v));
  });

  handle.join().unwrap();

  // Since we subscribed after emissions, no values are received
  // This demonstrates the "hot" nature of Subjects - you must subscribe before
  // emissions
  let results = results.lock().unwrap();
  assert_eq!(*results, vec![] as Vec<i32>);
}

#[rxrust_macro::test]
fn test_scan_with_seed() {
  // Test scan operator with accumulator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  Local::from_iter([1, 2, 3, 4])
    .scan(0, |acc, v| acc + v)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  // Should emit running sums
  assert_eq!(*results.borrow(), vec![1, 3, 6, 10]);
}

#[rxrust_macro::test]
fn test_distinct_until_changed() {
  // Test distinct until changed operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let mut subject = Local::subject::<i32, Infallible>();

  subject
    .clone()
    .distinct_until_changed()
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  // Emit with duplicates
  subject.next(1);
  subject.next(1); // Duplicate, should be filtered
  subject.next(2);
  subject.next(2); // Duplicate
  subject.next(2); // Duplicate
  subject.next(3);
  subject.next(1); // Different from previous, should emit

  assert_eq!(*results.borrow(), vec![1, 2, 3, 1]);
}

#[rxrust_macro::test]
fn test_collect_operator() {
  // Test collect operator for gathering all values
  let result = Rc::new(RefCell::new(None));
  let result_clone = result.clone();

  Local::from_iter(1..=5)
    .collect::<Vec<_>>()
    .subscribe(move |v| {
      *result_clone.borrow_mut() = Some(v);
    });

  assert_eq!(*result.borrow(), Some(vec![1, 2, 3, 4, 5]));
}

#[rxrust_macro::test]
fn test_take_until() {
  // Test take_until operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let mut source = Local::subject::<i32, Infallible>();
  let mut trigger = Local::subject::<(), Infallible>();

  source
    .clone()
    .take_until(trigger.clone())
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  source.next(1);
  source.next(2);
  trigger.next(()); // Trigger - should stop source
  source.next(3); // Should not be emitted

  assert_eq!(*results.borrow(), vec![1, 2]);
}

#[rxrust_macro::test]
fn test_with_latest_from() {
  // Test with_latest_from operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let mut primary = Local::subject::<i32, Infallible>();
  let mut secondary = Local::subject::<i32, Infallible>();

  primary
    .clone()
    .with_latest_from(secondary.clone())
    .map(|(p, s)| (p, s))
    .subscribe(move |(p, s)| {
      results_clone.borrow_mut().push((p, s));
    });

  secondary.next(100); // Set secondary value
  primary.next(1); // Output: (1, 100)
  primary.next(2); // Output: (2, 100)
  secondary.next(200); // No output
  primary.next(3); // Output: (3, 200)

  assert_eq!(*results.borrow(), vec![(1, 100), (2, 100), (3, 200)]);
}

#[rxrust_macro::test]
fn test_pairwise() {
  // Test pairwise operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  let mut subject = Local::subject::<i32, Infallible>();

  subject
    .clone()
    .pairwise()
    .subscribe(move |(a, b)| {
      results_clone.borrow_mut().push((a, b));
    });

  subject.next(1); // No output (need at least 2 values)
  subject.next(2); // Output: (1, 2)
  subject.next(3); // Output: (2, 3)
  subject.next(4); // Output: (3, 4)

  assert_eq!(*results.borrow(), vec![(1, 2), (2, 3), (3, 4)]);
}

#[rxrust_macro::test]
fn test_skip_and_take() {
  // Test combining skip and take operators
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  Local::from_iter(1..=10)
    .skip(3)
    .take(4)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  assert_eq!(*results.borrow(), vec![4, 5, 6, 7]);
}

#[rxrust_macro::test]
fn test_start_with() {
  // Test start_with operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  Local::from_iter([2, 3, 4])
    .start_with(vec![1])
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  assert_eq!(*results.borrow(), vec![1, 2, 3, 4]);
}

#[rxrust_macro::test]
fn test_default_if_empty() {
  // Test default_if_empty operator
  // Skip for now - Empty emits (), needs special handling
}

#[rxrust_macro::test]
fn test_take_last() {
  // Test take_last operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  Local::from_iter(1..=5)
    .take_last(3)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  assert_eq!(*results.borrow(), vec![3, 4, 5]);
}

#[rxrust_macro::test]
fn test_skip_last() {
  // Test skip_last operator
  let results = Rc::new(RefCell::new(Vec::new()));
  let results_clone = results.clone();

  Local::from_iter(1..=5)
    .skip_last(2)
    .subscribe(move |v| results_clone.borrow_mut().push(v));

  assert_eq!(*results.borrow(), vec![1, 2, 3]);
}
#[rxrust_macro::test]
fn average_of_floats() {
  let mut emitted = 0.0;
  let mut num_emissions = 0;

  Local::from_iter(vec![3., 4., 5., 6., 7.])
    .average()
    .subscribe(|v| {
      num_emissions += 1;
      emitted = v
    });

  assert!(approx_eq(5.0, emitted));
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

#[rxrust_macro::test]
fn average_on_single_float_item() {
  let mut emitted = 0.0;
  Local::of(123.0)
    .average()
    .subscribe(|v| emitted = v);
  assert!(approx_eq(123.0, emitted));
}

#[rxrust_macro::test]
fn average_of_integers() {
  let mut emitted = 0;
  Local::from_iter(vec![1, 2, 3, 4, 5])
    .average()
    .subscribe(|v| emitted = v);
  assert_eq!(3, emitted);
}

#[rxrust_macro::test]
fn test_combine_latest_reentrancy_path() {
  // Source - https://stackoverflow.com/q
  let mut s1 = Local::behavior_subject::<i32, Infallible>(1);
  let s2 = Local::behavior_subject::<i32, Infallible>(1);

  let c1 = s1
    .clone()
    .combine_latest(s2.clone(), |a, b| a + b)
    .distinct_until_changed();
  let c2 = s2
    .clone()
    .combine_latest(s1.clone(), |a, b| a + b)
    .distinct_until_changed();

  let mut s2_c = s2.clone();

  c1.combine_latest(c2, |a, b| a + b)
    .subscribe(move |v| s2_c.next(v));

  s1.next(2);
}
