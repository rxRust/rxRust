//! Sample operator implementation
//!
//! This module contains the Sample operator, which emits the most recently
//! emitted value from the source Observable whenever a notifier Observable
//! emits.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{Subscription, TupleSubscription},
};

// ==================== Sample Operator ====================

/// Sample operator
///
/// Emits the most recently emitted value from the source Observable whenever
/// the `sampler` (notifier) Observable emits a value. Also emits the last
/// stored value when the sampler completes.
#[derive(Clone)]
pub struct Sample<S, N> {
  pub source: S,
  pub sampler: N,
}

impl<S, N> ObservableType for Sample<S, N>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== Shared State ====================

/// Shared state between source and sampler observers
///
/// Contains the downstream observer and the latest sampled value.
/// Users should wrap this in `MutRc` or `MutArc` to share between observers.
pub struct SampleState<O, V> {
  observer: Option<O>,
  value: Option<V>,
}

impl<O, V> SampleState<O, V> {
  /// Create a new SampleState with the given observer
  pub fn new(observer: O) -> Self { Self { observer: Some(observer), value: None } }

  /// Store a value for later sampling
  #[inline]
  pub fn store(&mut self, value: V) { self.value = Some(value); }

  /// Take and emit the stored value if present
  pub fn emit_if_present<Err>(&mut self)
  where
    O: Observer<V, Err>,
  {
    if let Some(value) = self.value.take()
      && let Some(observer) = self.observer.as_mut()
    {
      observer.next(value);
    }
  }

  /// Propagate error to downstream and consume the observer
  pub fn error<Err>(&mut self, err: Err)
  where
    O: Observer<V, Err>,
  {
    if let Some(observer) = self.observer.take() {
      observer.error(err);
    }
  }

  /// Complete downstream and consume the observer
  pub fn complete<Err>(&mut self)
  where
    O: Observer<V, Err>,
  {
    if let Some(observer) = self.observer.take() {
      observer.complete();
    }
  }

  /// Check if downstream is closed
  pub fn is_closed<Err>(&self) -> bool
  where
    O: Observer<V, Err>,
  {
    self.observer.is_closed()
  }
}

// ==================== Observer Structs ====================

/// Observer for the source observable
pub struct SampleSourceObserver<StateRc, NProxy> {
  state: StateRc,
  notifier_proxy: NProxy,
}

/// Observer for the sampler (notifier) observable
pub struct SampleSamplerObserver<StateRc> {
  state: StateRc,
}

// ==================== Type Aliases ====================

/// Helper type alias for the shared state wrapped in RcMut
type SharedState<'a, C, S> =
  <C as Context>::RcMut<SampleState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>;

/// Helper type alias for the Sample source observable context
type SampleSourceCtx<'a, C, S, U> = <C as Context>::With<
  SampleSourceObserver<SharedState<'a, C, S>, <C as Context>::RcMut<Option<U>>>,
>;

/// Helper type alias for the Sample sampler observable context
type SampleSamplerCtx<'a, C, S> =
  <C as Context>::With<SampleSamplerObserver<SharedState<'a, C, S>>>;

// ==================== CoreObservable Implementation ====================

impl<S, N, C, SrcUnsub, NotifyUnsub> CoreObservable<C> for Sample<S, N>
where
  C: Context,
  S: for<'a> CoreObservable<SampleSourceCtx<'a, C, S, NotifyUnsub>, Unsub = SrcUnsub>,
  N: for<'a> CoreObservable<SampleSamplerCtx<'a, C, S>, Unsub = NotifyUnsub>,
  SrcUnsub: Subscription,
  NotifyUnsub: Subscription,
  C::RcMut<Option<NotifyUnsub>>: Subscription,
  for<'a> C::Inner: Observer<S::Item<'a>, S::Err>,
{
  type Unsub = TupleSubscription<SrcUnsub, C::RcMut<Option<NotifyUnsub>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Sample { source, sampler } = self;

    // Create shared state wrapped in RcMut
    let downstream = context.into_inner();
    let state: SharedState<C, S> = C::RcMut::from(SampleState::new(downstream));

    // Initialize notifier proxy (empty initially)
    let notifier_proxy: C::RcMut<Option<N::Unsub>> = C::RcMut::from(None);

    // Subscribe to source first
    let source_observer =
      SampleSourceObserver { state: state.clone(), notifier_proxy: notifier_proxy.clone() };
    let source_ctx = C::lift(source_observer);
    let source_unsub = source.subscribe(source_ctx);

    // Subscribe to sampler second
    let sampler_observer = SampleSamplerObserver { state: state.clone() };
    let sampler_ctx = C::lift(sampler_observer);
    let sampler_unsub = sampler.subscribe(sampler_ctx);

    // If downstream is not closed, store the sampler subscription in the proxy.
    // Otherwise, unsubscribe the sampler immediately (as source already
    // completed/errored).
    if !state.rc_deref().is_closed::<S::Err>() {
      *notifier_proxy.rc_deref_mut() = Some(sampler_unsub);
    } else {
      sampler_unsub.unsubscribe();
    }

    TupleSubscription::new(source_unsub, notifier_proxy)
  }
}

// ==================== Observer Implementations ====================

impl<Item, Err, O, StateRc, NProxy> Observer<Item, Err> for SampleSourceObserver<StateRc, NProxy>
where
  StateRc: RcDerefMut<Target = SampleState<O, Item>>,
  NProxy: Subscription,
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.state.rc_deref_mut().store(value); }

  fn error(self, err: Err) {
    self.notifier_proxy.unsubscribe();
    self.state.rc_deref_mut().error(err);
  }

  fn complete(self) {
    self.notifier_proxy.unsubscribe();
    self.state.rc_deref_mut().complete::<Err>();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().is_closed::<Err>() }
}

impl<Item, Err, SamplerItem, O, StateRc> Observer<SamplerItem, Err>
  for SampleSamplerObserver<StateRc>
where
  StateRc: RcDerefMut<Target = SampleState<O, Item>>,
  O: Observer<Item, Err>,
{
  fn next(&mut self, _: SamplerItem) { self.state.rc_deref_mut().emit_if_present::<Err>(); }

  fn error(self, err: Err) { self.state.rc_deref_mut().error(err); }

  fn complete(self) {
    let mut state = self.state.rc_deref_mut();
    state.emit_if_present::<Err>();
    state.complete::<Err>();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().is_closed::<Err>() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_sample_emits_on_notifier() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let mut sampler = Local::subject::<(), Infallible>();

    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    sampler.next(()); // Should emit 2
    source.next(3);
    sampler.next(()); // Should emit 3

    assert_eq!(*result.borrow(), vec![2, 3]);
  }

  #[rxrust_macro::test]
  fn test_sample_no_value_when_sampler_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let source = Local::subject::<i32, Infallible>();
    let mut sampler = Local::subject::<(), Infallible>();

    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Sampler emits before source has any value
    sampler.next(());
    sampler.next(());

    assert!(result.borrow().is_empty());
  }

  #[rxrust_macro::test]
  fn test_sample_emits_on_sampler_complete() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let sampler = Local::subject::<(), Infallible>();

    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    sampler.complete(); // Should emit 2 and complete

    assert_eq!(*result.borrow(), vec![2]);
  }

  #[rxrust_macro::test]
  fn test_sample_source_complete_completes_downstream() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let sampler = Local::subject::<(), Infallible>();

    source
      .clone()
      .sample(sampler.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    source.complete();

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_sample_each_sampler_takes_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let mut sampler = Local::subject::<(), Infallible>();

    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    sampler.next(()); // Emits 1, clears value
    sampler.next(()); // No value to emit
    source.next(2);
    sampler.next(()); // Emits 2

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_sample_behavior_subject_source_and_sampler() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // BehaviorSubject(1) emits 1 immediately on subscription
    let mut source = Local::behavior_subject(1);
    // BehaviorSubject(()) emits () immediately on subscription
    let mut sampler = Local::behavior_subject(());

    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Should emit 1 immediately because source emits 1 (stored) then sampler emits
    // (samples 1) If we subscribed sampler first: sampler emits, source not
    // subbed (no value), then source emits 1. Result: empty.
    assert_eq!(*result.borrow(), vec![1]);

    source.next(2);
    sampler.next(());
    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
