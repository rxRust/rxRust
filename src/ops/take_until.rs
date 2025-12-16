//! TakeUntil operator implementation
//!
//! This module contains the TakeUntil operator, which emits values from the
//! source Observable until a second Observable (the notifier) emits a value.

use crate::{
  context::{Context, RcDeref},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{Subscription, TupleSubscription},
};

/// TakeUntil operator
///
/// Emits the values emitted by the source Observable until a `notifier`
/// Observable emits a value.
#[derive(Clone)]
pub struct TakeUntil<S, N> {
  pub source: S,
  pub notifier: N,
}

/// Observer for the source observable
pub struct TakeUntilObserver<ObserverRc, NProxy> {
  observer_rc: ObserverRc,
  notifier_proxy: NProxy,
}

/// Observer for the notifier observable
///
/// Uses a function pointer to erase Item/Err type dependencies while
/// maintaining the ability to call complete() on the underlying Observer.
pub struct TakeUntilNotifierObserver<ObserverRc> {
  observer_rc: ObserverRc,
  complete_fn: fn(ObserverRc),
}

impl<ObserverRc: Clone> TakeUntilNotifierObserver<ObserverRc> {
  /// Creates a new TakeUntilNotifierObserver
  ///
  /// The Item and Err type parameters are used to satisfy Observer bounds
  /// during construction, but are erased via function pointers.
  pub fn new<Item, Err>(observer_rc: ObserverRc) -> Self
  where
    ObserverRc: Observer<Item, Err>,
  {
    Self { observer_rc, complete_fn: |o| o.complete() }
  }
}

impl<S, N> ObservableType for TakeUntil<S, N>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, N, C> CoreObservable<C> for TakeUntil<S, N>
where
  C: Context,
  S: CoreObservable<
    C::With<TakeUntilObserver<C::RcMut<Option<C::Inner>>, C::RcMut<Option<N::Unsub>>>>,
  >,
  N: CoreObservable<C::With<TakeUntilNotifierObserver<C::RcMut<Option<C::Inner>>>>>,
  C::RcMut<Option<N::Unsub>>: Subscription,
  for<'a> C::RcMut<Option<C::Inner>>: Observer<S::Item<'a>, S::Err>,
{
  type Unsub = TupleSubscription<S::Unsub, C::RcMut<Option<N::Unsub>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let TakeUntil { source, notifier } = self;

    // Create shared state holding the downstream observer
    let downstream = context.into_inner();
    let observer_rc = C::RcMut::from(Some(downstream));

    // Subscribe to notifier - use ::new to erase Item/Err type dependencies
    let notifier_observer =
      TakeUntilNotifierObserver::new::<S::Item<'_>, S::Err>(observer_rc.clone());
    let notifier_ctx = C::lift(notifier_observer);
    let notifier_unsub = notifier.subscribe(notifier_ctx);
    let notifier_proxy: C::RcMut<Option<N::Unsub>> = C::RcMut::from(Some(notifier_unsub));

    // Subscribe to source
    let source_observer = TakeUntilObserver { observer_rc, notifier_proxy: notifier_proxy.clone() };
    // Create context for source using original scheduler
    let source_ctx = C::lift(source_observer);
    let source_unsub = source.subscribe(source_ctx);

    // Return tuple subscription with direct source subscription and notifier
    // subscription
    TupleSubscription::new(source_unsub, notifier_proxy)
  }
}

// Implement Observer for Source
impl<Item, Err, ObserverRc, NProxy> Observer<Item, Err> for TakeUntilObserver<ObserverRc, NProxy>
where
  ObserverRc: Observer<Item, Err> + Clone,
  NProxy: Subscription,
{
  fn next(&mut self, value: Item) { self.observer_rc.clone().next(value); }

  fn error(self, err: Err) {
    self.observer_rc.clone().error(err);
    self.notifier_proxy.unsubscribe();
  }

  fn complete(self) {
    self.observer_rc.clone().complete();
    self.notifier_proxy.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.observer_rc.is_closed() }
}

// Implement Observer for Notifier - no longer needs Item/Err bounds
// Uses RcDeref to check Option::is_none() for is_closed
impl<NotifyItem, NotifyErr, ObserverRc, O> Observer<NotifyItem, NotifyErr>
  for TakeUntilNotifierObserver<ObserverRc>
where
  ObserverRc: Clone + RcDeref<Target = Option<O>>,
{
  fn next(&mut self, _value: NotifyItem) { (self.complete_fn)(self.observer_rc.clone()); }

  fn error(self, _err: NotifyErr) {
    // Ignore errors from notifier as per legacy behavior
  }

  fn complete(self) {
    // Ignore completion from notifier
  }

  fn is_closed(&self) -> bool { self.observer_rc.rc_deref().is_none() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_take_until_emits_until_notifier_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    notifier.next(());
    source.next(3);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_take_until_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    notifier.next(());

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_until_source_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    source.complete();

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_until_notifier_complete_does_nothing() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.next(1);
    notifier.complete(); // Should be ignored
    source.next(2);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
