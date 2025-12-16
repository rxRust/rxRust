//! Finalize operator implementation
//!
//! This module contains the Finalize operator, which calls a function
//! when the Observable completes, errors, or is unsubscribed.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Finalize operator: Calls a function on complete, error, or unsubscribe
///
/// The `finalize` operator allows you to execute cleanup logic when the
/// Observable terminates for any reason (complete, error, or unsubscribe).
/// The finalize function is guaranteed to be called exactly once.
///
/// # Examples
///
/// ```
/// use std::{cell::Cell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let finalized = Rc::new(Cell::new(false));
/// let finalized_clone = finalized.clone();
/// Local::of(1)
///   .finalize(move || finalized_clone.set(true))
///   .subscribe(|_| {});
/// assert!(finalized.get());
/// ```
#[derive(Clone)]
pub struct Finalize<S, F> {
  pub source: S,
  pub func: F,
}

/// FinalizeObserver wrapper for cleanup on complete/error
///
/// This observer wraps another observer and calls a finalize function
/// when `complete` or `error` is called. The function is wrapped in
/// an `Option` and taken once to ensure single execution.
pub struct FinalizeObserver<O, P> {
  observer: O,
  func: P,
}

impl<O, P, F, Item, Err> Observer<Item, Err> for FinalizeObserver<O, P>
where
  O: Observer<Item, Err>,
  P: RcDerefMut<Target = Option<F>>,
  F: FnOnce(),
{
  fn next(&mut self, v: Item) { self.observer.next(v); }

  fn error(self, e: Err) {
    self.observer.error(e);
    if let Some(func) = self.func.rc_deref_mut().take() {
      func();
    }
  }

  fn complete(self) {
    self.observer.complete();
    if let Some(func) = self.func.rc_deref_mut().take() {
      func();
    }
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F> ObservableType for Finalize<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Subscription wrapper that calls finalize on unsubscribe
pub struct FinalizeSubscription<U, P> {
  subscription: U,
  func: P,
}

impl<U, P, F> Subscription for FinalizeSubscription<U, P>
where
  U: Subscription,
  P: RcDerefMut<Target = Option<F>>,
  F: FnOnce(),
{
  fn unsubscribe(self) {
    self.subscription.unsubscribe();
    if let Some(func) = self.func.rc_deref_mut().take() {
      func();
    }
  }

  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<S, F, C> CoreObservable<C> for Finalize<S, F>
where
  C: Context,
  S: CoreObservable<C::With<FinalizeObserver<C::Inner, C::RcMut<Option<F>>>>>,
  F: FnOnce(),
{
  type Unsub = FinalizeSubscription<S::Unsub, C::RcMut<Option<F>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Finalize { source, func } = self;
    let func_ptr: C::RcMut<Option<F>> = C::RcMut::from(Some(func));
    let func_clone = func_ptr.clone();
    let wrapped = context.transform(|observer| FinalizeObserver { observer, func: func_ptr });
    let subscription = source.subscribe(wrapped);
    FinalizeSubscription { subscription, func: func_clone }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
  };

  use crate::{prelude::*, subscription::Subscription};

  #[rxrust_macro::test]
  fn test_finalize_on_complete_simple() {
    let finalized = Rc::new(Cell::new(false));
    let mut nexted = false;
    let finalized_clone = finalized.clone();
    Local::of(1)
      .finalize(move || finalized_clone.set(true))
      .subscribe(|_| nexted = true);
    assert!(finalized.get());
    assert!(nexted);
  }

  #[rxrust_macro::test]
  fn test_finalize_on_complete_subject() {
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let subject: Local<_> = Local::subject();
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    subject
      .clone()
      .finalize(move || finalized_clone.set(true))
      .subscribe(move |_: i32| nexted_clone.set(true));
    subject.clone().next(1);
    subject.clone().next(2);
    subject.complete();
    assert!(finalized.get());
    assert!(nexted.get());
  }

  #[rxrust_macro::test]
  fn test_finalize_on_unsubscribe() {
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let subject: Local<_> = Local::subject();
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let subscription = subject
      .clone()
      .finalize(move || finalized_clone.set(true))
      .subscribe(move |_: i32| nexted_clone.set(true));
    subject.clone().next(1);
    subject.clone().next(2);
    subscription.unsubscribe();
    assert!(finalized.get());
    assert!(nexted.get());
  }

  #[rxrust_macro::test]
  fn test_finalize_on_error() {
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let errored = Rc::new(Cell::new(false));
    let subject: Local<_> = Local::subject::<i32, &'static str>();
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let errored_clone = errored.clone();
    subject
      .clone()
      .finalize(move || finalized_clone.set(true))
      .on_error(move |_| errored_clone.set(true))
      .subscribe(move |_| nexted_clone.set(true));
    subject.clone().next(1);
    subject.clone().next(2);
    subject.error("oops");
    assert!(finalized.get());
    assert!(errored.get());
    assert!(nexted.get());
  }

  #[rxrust_macro::test]
  fn test_finalize_only_once() {
    let finalize_count = Rc::new(Cell::new(0));
    let subject: Local<_> = Local::subject::<i32, &'static str>();
    let finalized_clone = finalize_count.clone();
    let subscription = subject
      .clone()
      .finalize(move || finalized_clone.set(finalized_clone.get() + 1))
      .on_error(|_| {})
      .subscribe(|_| {});
    subject.clone().next(1);
    subject.clone().next(2);
    subject.clone().error("oops");
    subscription.unsubscribe();
    assert_eq!(finalize_count.get(), 1);
  }

  #[rxrust_macro::test]
  fn test_finalize_chaining() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let order_clone1 = order.clone();
    let order_clone2 = order.clone();
    Local::of(1)
      .finalize(move || order_clone1.borrow_mut().push(1))
      .finalize(move || order_clone2.borrow_mut().push(2))
      .subscribe(|_| {});
    // Outer finalize (push 2) is called first due to observer chain order,
    // then inner finalize (push 1) is called
    assert_eq!(*order.borrow(), vec![2, 1]);
  }
}
