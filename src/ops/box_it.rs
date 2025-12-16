//! BoxIt operator implementation
//!
//! This module provides the `box_it()` operator for type-erasing observables.
//! The actual boxed types are defined in `observable::boxed`.

// Re-export the boxed types for backwards compatibility
pub use crate::observable::boxed::{
  BoxedCoreObservable, BoxedCoreObservableClone, BoxedCoreObservableMutRef,
  BoxedCoreObservableMutRefClone, BoxedCoreObservableMutRefSend,
  BoxedCoreObservableMutRefSendClone, BoxedCoreObservableSend, BoxedCoreObservableSendClone,
  DynCoreObservable, DynCoreObservableClone, IntoBoxedCoreObservable,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{observable::boxed::*, prelude::*, scheduler::LocalScheduler};

  #[rxrust_macro::test]
  fn test_box_it_observable_method() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // Use box_it() method on Observable trait directly
    Local::of(42).box_it().subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_box_it_clone_observable_method() {
    let result = Rc::new(RefCell::new(Vec::new()));

    let boxed = Local::of(42).box_it_clone();
    let boxed2 = boxed.clone();

    {
      let result_clone = result.clone();
      boxed.subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });
    }
    {
      let result_clone = result.clone();
      boxed2.subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });
    }

    assert_eq!(*result.borrow(), vec![42, 42]);
  }

  #[rxrust_macro::test]
  fn test_local_box_observable() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // Box an observable - note: Of<T> has Err = Infallible
    let boxed: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> =
      Local::of(42).into_inner().into_boxed();

    // Subscribe to the boxed observable
    Local::new(boxed).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_heterogeneous_collection() {
    let result = Rc::new(RefCell::new(Vec::new()));

    // Create boxed observables from different sources - all sources have Err =
    // Infallible
    let boxed1: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> =
      Local::of(1).into_inner().into_boxed();
    let boxed2: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> =
      Local::from_iter([2, 3]).into_inner().into_boxed();

    // Store in a Vec
    let observables = vec![boxed1, boxed2];

    // Subscribe to each
    for obs in observables {
      let result_clone = result.clone();
      Local::new(obs).subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });
    }

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_type_erasure() {
    // Different source types result in the same boxed type - all with Err =
    // Infallible
    let _boxed1: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> =
      Local::of(1).into_inner().into_boxed();

    let _boxed2: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> = Local::of(2)
      .map(|x| x * 2)
      .into_inner()
      .into_boxed();

    let _boxed3: BoxedCoreObservable<'_, i32, Infallible, LocalScheduler> =
      Local::from_iter([1, 2, 3])
        .filter(|x| *x > 1)
        .into_inner()
        .into_boxed();

    // All have the same type - compilation success proves type erasure works
  }

  #[rxrust_macro::test]
  fn test_box_it_with_type_alias() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // Use LocalBoxedObservable type alias
    let boxed: LocalBoxedObservable<'_, i32, Infallible> = Local::of(42).box_it();
    boxed.subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_box_it_heterogeneous_with_type_alias() {
    let result = Rc::new(RefCell::new(Vec::new()));

    // Different source types become the same LocalBoxedObservable type
    let boxed1: LocalBoxedObservable<'_, i32, Infallible> = Local::of(1).box_it();
    let boxed2: LocalBoxedObservable<'_, i32, Infallible> =
      Local::from_iter([2, 3]).map(|x| x * 2).box_it();

    // Store in a collection
    let observables = vec![boxed1, boxed2];

    for obs in observables {
      let result_clone = result.clone();
      obs.subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });
    }

    assert_eq!(*result.borrow(), vec![1, 4, 6]);
  }
}
