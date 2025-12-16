//! Defer operator implementation
//!
//! This module contains the Defer operator, which creates an observable that
//! calls an observable factory to make an observable for each new observer.
//! This allows for lazy creation of observables at subscription time.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
};

/// Defer operator: Creates an observable that calls an observable factory
///
/// This operator creates an observable that, on subscription, calls an
/// observable factory function to make an Observable for each new Observer.
/// This allows for lazy creation of observables and ensures that the factory
/// function is called each time someone subscribes.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::defer(|| {
///   println!("Creating observable!");
///   Local::of("Hello!")
/// });
///
/// observable.subscribe(|v| {
///   println!("{}", v);
/// });
/// // Prints: Creating observable!
/// //         Hello!
/// ```
pub struct Defer<F, S> {
  pub func: F,
  _marker: std::marker::PhantomData<S>,
}

impl<F, S> Defer<F, S> {
  pub fn new(func: F) -> Self { Self { func, _marker: std::marker::PhantomData } }
}

impl<F, S> ObservableType for Defer<F, S>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<F, S, C> CoreObservable<C> for Defer<F, S>
where
  C: Context,
  F: FnOnce() -> C::With<S>,
  S: CoreObservable<C>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub { (self.func)().into_inner().subscribe(context) }
}

impl<F: Clone, S> Clone for Defer<F, S> {
  fn clone(&self) -> Self { Self { func: self.func.clone(), _marker: std::marker::PhantomData } }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_defer_calls_factory_on_subscribe() {
    let calls = Rc::new(RefCell::new(0));
    let calls_clone = calls.clone();

    let observable = Local::defer(move || {
      *calls_clone.borrow_mut() += 1;
      Local::of(42)
    });

    // Before subscription, no calls should have been made
    assert_eq!(*calls.borrow(), 0);

    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    observable.clone().subscribe(move |v| {
      *result_clone.borrow_mut() = Some(v);
    });

    // After first subscription, exactly one call should have been made
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(*result.borrow(), Some(42));

    // Subscribe again
    let result2 = Rc::new(RefCell::new(None));
    let result2_clone = result2.clone();

    observable.clone().subscribe(move |v| {
      *result2_clone.borrow_mut() = Some(v);
    });

    // After second subscription, exactly two calls should have been made
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(*result2.borrow(), Some(42));
  }

  #[rxrust_macro::test(local)]
  async fn test_defer_with_different_observables() {
    let counter = Rc::new(RefCell::new(0));
    let counter_clone = counter.clone();

    let observable = Local::defer(move || {
      let value = *counter_clone.borrow();
      *counter_clone.borrow_mut() += 1;
      Local::of(value)
    });

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    observable.clone().subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    // First subscription should get 0
    assert_eq!(*result.borrow(), vec![0]);

    // Second subscription should get 1
    let result2 = Rc::new(RefCell::new(Vec::new()));
    let result2_clone = result2.clone();

    observable.clone().subscribe(move |v| {
      result2_clone.borrow_mut().push(v);
    });

    assert_eq!(*result2.borrow(), vec![1]);
  }

  #[rxrust_macro::test(local)]
  async fn test_defer_with_error() {
    let observable = Local::defer(|| Local::throw_err("Test error"));

    let error = Rc::new(RefCell::new(None));
    let error_clone = error.clone();

    observable
      .on_error(move |e| {
        *error_clone.borrow_mut() = Some(e);
      })
      .subscribe(|_| panic!("Should not receive any values"));

    assert_eq!(*error.borrow(), Some("Test error"));
  }

  #[rxrust_macro::test(local)]
  async fn test_defer_multiple_subscriptions() {
    let calls = Rc::new(RefCell::new(0));
    let calls_clone = calls.clone();

    let observable = Local::defer(move || {
      *calls_clone.borrow_mut() += 1;
      Local::of(10)
    });

    // Subscribe twice by cloning the observable
    let result1 = Rc::new(RefCell::new(None));
    let result1_clone = result1.clone();
    let result2 = Rc::new(RefCell::new(None));
    let result2_clone = result2.clone();

    observable.clone().subscribe(move |v| {
      *result1_clone.borrow_mut() = Some(v);
    });

    observable.clone().subscribe(move |v| {
      *result2_clone.borrow_mut() = Some(v);
    });

    // Both should receive the value and 2 calls should have been made
    assert_eq!(*result1.borrow(), Some(10));
    assert_eq!(*result2.borrow(), Some(10));
    assert_eq!(*calls.borrow(), 2);
  }

  #[rxrust_macro::test]
  async fn test_defer_with_shared_context() {
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_clone = result.clone();

    Shared::defer(|| Shared::of("deferred value")).subscribe(move |v| {
      *result_clone.lock().unwrap() = Some(v);
    });

    assert_eq!(*result.lock().unwrap(), Some("deferred value"));
  }
}
