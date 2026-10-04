//! Sample operator implementation
//!
//! This module contains the Sample operator, which emits the most recently
//! emitted value from the source Observable whenever a notifier Observable
//! emits.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, TupleSubscription, single_assignment::State},
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
pub struct SampleSamplerObserver<StateRc, H> {
  source: H,
  state: StateRc,
}

// ==================== Type Aliases ====================

/// Helper type alias for the shared state wrapped in RcMut
type SharedState<'a, C, S> =
  <C as Context>::RcMut<SampleState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>;

type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type SourceCtx<'a, C, S, H> = <C as Context>::With<SampleSourceObserver<SharedState<'a, C, S>, H>>;
type SamplerCtx<'a, C, S, H> =
  <C as Context>::With<SampleSamplerObserver<SharedState<'a, C, S>, H>>;
impl<S, N, C, U, V> CoreObservable<C> for Sample<S, N>
where
  C: Context,
  U: Subscription,
  V: Subscription,
  S: for<'a> CoreObservable<SourceCtx<'a, C, S, ()>, Unsub = U>
    + for<'a> CoreObservable<SourceCtx<'a, C, S, Handle<C, V>>, Unsub = U>,
  N: for<'a> CoreObservable<SamplerCtx<'a, C, S, Handle<C, U>>, Unsub = V>,
{
  type Unsub = TupleSubscription<Handle<C, U>, Handle<C, V>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let (observer, scheduler) = context.into_parts();
    let state = C::RcMut::from(SampleState::new(observer));
    let source = Handle::<C, U>::new();
    let sampler = Handle::<C, V>::new();
    source.set(self.source.subscribe(C::With::from_parts(
      SampleSourceObserver { state: state.clone(), notifier_proxy: sampler.clone() },
      scheduler.clone(),
    )));
    if state.rc_deref().observer.is_some() {
      sampler.set(self.sampler.subscribe(C::With::from_parts(
        SampleSamplerObserver { state, source: source.clone() },
        scheduler,
      )));
    }
    TupleSubscription::new(source, sampler)
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
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    self.notifier_proxy.unsubscribe();
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    self.notifier_proxy.unsubscribe();
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().is_closed::<Err>() }
}

impl<Item, Err, SamplerItem, O, StateRc, H> Observer<SamplerItem, Err>
  for SampleSamplerObserver<StateRc, H>
where
  H: Subscription,
  StateRc: RcDerefMut<Target = SampleState<O, Item>>,
  O: Observer<Item, Err>,
{
  fn next(&mut self, _: SamplerItem) { self.state.rc_deref_mut().emit_if_present::<Err>(); }

  fn error(self, err: Err) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.source.unsubscribe();
  }

  fn complete(self) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let (observer, value) = {
      let mut state = self.state.rc_deref_mut();
      (state.observer.take(), state.value.take())
    };
    if let Some(mut observer) = observer {
      if let Some(value) = value {
        observer.next(value);
      }
      observer.complete();
    }
    self.source.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().is_closed::<Err>() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {

  #[rxrust_macro::test(local)]
  async fn sampler_completion_cancels_source() {
    let mut source = Local::subject::<i32, Infallible>();
    let sampler = Local::subject::<(), Infallible>();
    let result = Rc::new(RefCell::new(vec![]));
    let r = result.clone();
    source
      .clone()
      .sample(sampler.clone())
      .subscribe(move |v| r.borrow_mut().push(v));
    source.next(4);
    sampler.complete();
    assert_eq!(*result.borrow(), vec![4]);
    assert_eq!(source.inner().subscriber_count(), 0);
  }
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

    // Should emit 1 immediately because source emits 1 (stored) then sampler
    // emits (samples 1) If we subscribed sampler first: sampler emits,
    // source not subbed (no value), then source emits 1. Result: empty.
    assert_eq!(*result.borrow(), vec![1]);

    source.next(2);
    sampler.next(());
    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
