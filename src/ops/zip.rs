//! Zip operator implementation
//!
//! Zip combines items from two observables pairwise, emitting a tuple when
//! both sources have emitted a value.

use std::collections::VecDeque;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

// ==================== Zip Operator ====================

/// Zip operator
///
/// Combines items from two observables pairwise. It buffers items from each
/// source and emits a tuple `(ItemA, ItemB)` when both sources have emitted
/// a value. Completes when both sources complete.
#[derive(Clone)]
pub struct Zip<A, B> {
  pub source_a: A,
  pub source_b: B,
}

impl<A, B> ObservableType for Zip<A, B>
where
  A: ObservableType,
  B: ObservableType<Err = A::Err>,
{
  type Item<'a>
    = (A::Item<'a>, B::Item<'a>)
  where
    Self: 'a;
  type Err = A::Err;
}

// ==================== Shared State ====================

/// Shared state between A and B observers
pub struct ZipState<O, ItemA, ItemB> {
  observer: Option<O>,
  buffer_a: VecDeque<ItemA>,
  buffer_b: VecDeque<ItemB>,
  completed_a: bool,
  completed_b: bool,
}

impl<O, ItemA, ItemB> ZipState<O, ItemA, ItemB> {
  fn new(observer: O) -> Self {
    Self {
      observer: Some(observer),
      buffer_a: VecDeque::new(),
      buffer_b: VecDeque::new(),
      completed_a: false,
      completed_b: false,
    }
  }

  /// Check and trigger completion if conditions are met
  fn check_complete<E>(&mut self)
  where
    O: Observer<(ItemA, ItemB), E>,
  {
    // Complete when both sources complete, or when one completes and the other's
    // buffer is empty (no more pairs can be formed)
    let should_complete = (self.buffer_a.is_empty() || self.completed_b) && self.completed_a
      || (self.buffer_b.is_empty() || self.completed_a) && self.completed_b;

    if should_complete && let Some(observer) = self.observer.take() {
      observer.complete();
    }
  }
}

// ==================== Observer Structs ====================

/// Observer for source A
pub struct ZipAObserver<StateRc, BProxy> {
  state: StateRc,
  b_proxy: BProxy,
}

/// Observer for source B
pub struct ZipBObserver<StateRc, AProxy> {
  state: StateRc,
  a_proxy: AProxy,
}

// ==================== Type Aliases ====================

type SharedState<'a, C, A, B> = <C as Context>::RcMut<
  ZipState<<C as Context>::Inner, <A as ObservableType>::Item<'a>, <B as ObservableType>::Item<'a>>,
>;

type SubProxy<C, U> = <C as Context>::RcMut<Option<U>>;
type BoxedSubProxy<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;

type ZipACtx<'a, C, A, B, BUnsub> =
  <C as Context>::With<ZipAObserver<SharedState<'a, C, A, B>, SubProxy<C, BUnsub>>>;

type ZipBCtx<'a, C, A, B> =
  <C as Context>::With<ZipBObserver<SharedState<'a, C, A, B>, BoxedSubProxy<C>>>;

// ==================== CoreObservable Implementation ====================

impl<A, B, C, AUnsub, BUnsub> CoreObservable<C> for Zip<A, B>
where
  C: Context,
  A: ObservableType + for<'a> CoreObservable<ZipACtx<'a, C, A, B, BUnsub>, Unsub = AUnsub>,
  B: ObservableType<Err = A::Err> + for<'a> CoreObservable<ZipBCtx<'a, C, A, B>, Unsub = BUnsub>,
  AUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SubProxy<C, BUnsub>: Subscription,
  BoxedSubProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<BoxedSubProxy<C>, SubProxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Zip { source_a, source_b } = self;

    let downstream = context.into_inner();
    let state: SharedState<C, A, B> = C::RcMut::from(ZipState::new(downstream));

    let a_proxy: BoxedSubProxy<C> = C::RcMut::from(None);
    let b_proxy: SubProxy<C, BUnsub> = C::RcMut::from(None);

    let b_observer = ZipBObserver { state: state.clone(), a_proxy: a_proxy.clone() };
    let b_ctx = C::lift(b_observer);
    let b_unsub = source_b.subscribe(b_ctx);
    *b_proxy.rc_deref_mut() = Some(b_unsub);

    let a_observer = ZipAObserver { state, b_proxy: b_proxy.clone() };
    let a_ctx = C::lift(a_observer);
    let a_unsub = source_a.subscribe(a_ctx);
    *a_proxy.rc_deref_mut() = Some(a_unsub.into_boxed());

    TupleSubscription::new(a_proxy, b_proxy)
  }
}

// ==================== Observer Implementations ====================

impl<ItemA, ItemB, Err, O, StateRc, BProxy> Observer<ItemA, Err> for ZipAObserver<StateRc, BProxy>
where
  StateRc: RcDerefMut<Target = ZipState<O, ItemA, ItemB>>,
  BProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
{
  fn next(&mut self, value: ItemA) {
    let mut state = self.state.rc_deref_mut();
    if let Some(b) = state.buffer_b.pop_front() {
      if let Some(observer) = state.observer.as_mut() {
        observer.next((value, b));
      }
    } else {
      state.buffer_a.push_back(value);
    }
  }

  fn error(self, err: Err) {
    self.b_proxy.unsubscribe();
    if let Some(observer) = self.state.rc_deref_mut().observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.b_proxy.unsubscribe();
    let mut state = self.state.rc_deref_mut();
    state.completed_a = true;
    state.check_complete::<Err>();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

impl<ItemA, ItemB, Err, O, StateRc, AProxy> Observer<ItemB, Err> for ZipBObserver<StateRc, AProxy>
where
  StateRc: RcDerefMut<Target = ZipState<O, ItemA, ItemB>>,
  AProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
{
  fn next(&mut self, value: ItemB) {
    let mut state = self.state.rc_deref_mut();
    if let Some(a) = state.buffer_a.pop_front() {
      if let Some(observer) = state.observer.as_mut() {
        observer.next((a, value));
      }
    } else {
      state.buffer_b.push_back(value);
    }
  }

  fn error(self, err: Err) {
    self.a_proxy.unsubscribe();
    if let Some(observer) = self.state.rc_deref_mut().observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.a_proxy.unsubscribe();
    let mut state = self.state.rc_deref_mut();
    state.completed_b = true;
    state.check_complete::<Err>();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_zip_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .zip(Local::from_iter([4, 5, 6]))
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![(1, 4), (2, 5), (3, 6)]);
  }

  #[rxrust_macro::test(local)]
  async fn test_zip_different_lengths() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .zip(Local::from_iter([10, 20, 30]))
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![(1, 10), (2, 20), (3, 30)]);
  }

  #[rxrust_macro::test(local)]
  async fn test_zip_completion() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter([1, 2])
      .zip(Local::from_iter([3, 4]))
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: (i32, i32)| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_zip_with_subjects() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut subject_a = Local::subject::<i32, Infallible>();
    let mut subject_b = Local::subject::<i32, Infallible>();

    subject_a
      .clone()
      .zip(subject_b.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    subject_a.next(1);
    subject_a.next(2);
    subject_b.next(10);
    subject_b.next(20);
    subject_a.next(3);
    subject_b.next(30);

    assert_eq!(*result.borrow(), vec![(1, 10), (2, 20), (3, 30)]);

    subject_a.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_zip_sum() {
    let mut sum = 0;

    Local::from_iter(0..10)
      .zip(Local::from_iter(0..10))
      .map(|(a, b)| a + b)
      .subscribe(|v| sum += v);

    assert_eq!(sum, 90);
  }

  #[rxrust_macro::test(local)]
  async fn test_zip_count() {
    let mut count = 0;

    Local::from_iter(0..10)
      .zip(Local::from_iter(0..10))
      .subscribe(|_| count += 1);

    assert_eq!(count, 10);
  }
}
