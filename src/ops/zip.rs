//! Zip operator implementation
//!
//! Zip combines items from two observables pairwise, emitting a tuple when
//! both sources have emitted a value.

use std::collections::VecDeque;

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    IntoBoxedSubscription, SingleAssignment, Subscription, TupleSubscription,
    single_assignment::State,
  },
};

// ==================== Zip Operator ====================

/// Zip operator
///
/// Combines items from two observables pairwise. It buffers items from each
/// source and emits a tuple `(ItemA, ItemB)` when both sources have emitted
/// a value. Completes when a completed source has no buffered values left.
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

  fn take_completed(&mut self) -> Option<O> {
    if (self.completed_a && self.buffer_a.is_empty())
      || (self.completed_b && self.buffer_b.is_empty())
    {
      self.buffer_a.clear();
      self.buffer_b.clear();
      self.observer.take()
    } else {
      None
    }
  }
}

// ==================== Observer Structs ====================

/// Observer for source A
pub struct ZipAObserver<StateRc, BProxy, AProxy> {
  a_proxy: Option<AProxy>,
  state: StateRc,
  b_proxy: Option<BProxy>,
}

/// Observer for source B
pub struct ZipBObserver<StateRc, AProxy, BProxy> {
  b_proxy: Option<BProxy>,
  state: StateRc,
  a_proxy: Option<AProxy>,
}

// ==================== Type Aliases ====================

type SharedState<'a, C, A, B> = <C as Context>::RcMut<
  ZipState<<C as Context>::Inner, <A as ObservableType>::Item<'a>, <B as ObservableType>::Item<'a>>,
>;

type SubProxy<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type BoxedSubProxy<C> =
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>;

type ZipACtx<'a, C, A, B, BUnsub> = <C as Context>::With<
  ZipAObserver<SharedState<'a, C, A, B>, SubProxy<C, BUnsub>, BoxedSubProxy<C>>,
>;

type ZipBCtx<'a, C, A, B, H> =
  <C as Context>::With<ZipBObserver<SharedState<'a, C, A, B>, BoxedSubProxy<C>, H>>;

// ==================== CoreObservable Implementation ====================

impl<A, B, C, AUnsub, BUnsub> CoreObservable<C> for Zip<A, B>
where
  C: Context,
  A: ObservableType + for<'a> CoreObservable<ZipACtx<'a, C, A, B, BUnsub>, Unsub = AUnsub>,
  B: ObservableType<Err = A::Err>
    + for<'a> CoreObservable<ZipBCtx<'a, C, A, B, ()>, Unsub = BUnsub>
    + for<'a> CoreObservable<ZipBCtx<'a, C, A, B, SubProxy<C, BUnsub>>, Unsub = BUnsub>,
  BUnsub: Subscription,
  AUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SubProxy<C, BUnsub>: Subscription,
  BoxedSubProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<BoxedSubProxy<C>, SubProxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Zip { source_a, source_b } = self;

    let (downstream, scheduler) = context.into_parts();
    let state: SharedState<C, A, B> = C::RcMut::from(ZipState::new(downstream));

    let (install_a, [a_proxy, a_for_b, a_subscription]) = BoxedSubProxy::<C>::channel();
    let (install_b, [b_proxy, b_for_b, b_subscription]) = SubProxy::<C, BUnsub>::channel();

    let b_observer =
      ZipBObserver { state: state.clone(), a_proxy: Some(a_for_b), b_proxy: Some(b_for_b) };
    let b_ctx = C::With::from_parts(b_observer, scheduler.clone());
    let b_unsub = source_b.subscribe(b_ctx);
    install_b(b_unsub);

    if state.rc_deref().observer.is_some() {
      let a_observer = ZipAObserver { state, b_proxy: Some(b_proxy), a_proxy: Some(a_proxy) };
      let a_ctx = C::With::from_parts(a_observer, scheduler);
      let a_unsub = source_a.subscribe(a_ctx);
      install_a(a_unsub.into_boxed());
    }

    TupleSubscription::new(a_subscription, b_subscription)
  }
}

// ==================== Observer Implementations ====================

impl<ItemA, ItemB, Err, O, StateRc, BProxy, AProxy> Observer<ItemA, Err>
  for ZipAObserver<StateRc, BProxy, AProxy>
where
  StateRc: RcDerefMut<Target = ZipState<O, ItemA, ItemB>>,
  BProxy: Subscription,
  AProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
{
  fn next(&mut self, value: ItemA) {
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_none() {
      return;
    }
    if let Some(b) = state.buffer_b.pop_front() {
      if let Some(observer) = state.observer.as_mut() {
        observer.next((value, b));
      }
    } else {
      state.buffer_a.push_back(value);
    }
    let completed = state.take_completed();
    let active_a = !state.completed_a;
    let active_b = !state.completed_b;
    drop(state);
    if let Some(observer) = completed {
      observer.complete();
      if active_a {
        self.a_proxy.take().unsubscribe();
      }
      if active_b {
        self.b_proxy.take().unsubscribe();
      }
    }
  }

  fn error(self, err: Err) {
    let (observer, active) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_a = true;
      (state.observer.take(), !state.completed_b)
    };
    if let Some(observer) = observer {
      observer.error(err);
    }
    if active {
      self.b_proxy.unsubscribe();
    }
  }
  fn complete(self) {
    let (observer, active) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_a = true;
      (state.take_completed(), !state.completed_b)
    };
    if let Some(observer) = observer {
      observer.complete();
      if active {
        self.b_proxy.unsubscribe();
      }
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

impl<ItemA, ItemB, Err, O, StateRc, AProxy, BProxy> Observer<ItemB, Err>
  for ZipBObserver<StateRc, AProxy, BProxy>
where
  StateRc: RcDerefMut<Target = ZipState<O, ItemA, ItemB>>,
  BProxy: Subscription,
  AProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
{
  fn next(&mut self, value: ItemB) {
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_none() {
      return;
    }
    if let Some(a) = state.buffer_a.pop_front() {
      if let Some(observer) = state.observer.as_mut() {
        observer.next((a, value));
      }
    } else {
      state.buffer_b.push_back(value);
    }
    let completed = state.take_completed();
    let active_a = !state.completed_a;
    let active_b = !state.completed_b;
    drop(state);
    if let Some(observer) = completed {
      observer.complete();
      if active_a {
        self.a_proxy.take().unsubscribe();
      }
      if active_b {
        self.b_proxy.take().unsubscribe();
      }
    }
  }

  fn error(self, err: Err) {
    let (observer, active) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_b = true;
      (state.observer.take(), !state.completed_a)
    };
    if let Some(observer) = observer {
      observer.error(err);
    }
    if active {
      self.a_proxy.unsubscribe();
    }
  }
  fn complete(self) {
    let (observer, active) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_b = true;
      (state.take_completed(), !state.completed_a)
    };
    if let Some(observer) = observer {
      observer.complete();
      if active {
        self.a_proxy.unsubscribe();
      }
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {

  #[rxrust_macro::test(local)]
  async fn completed_buffer_drains_then_cancels_remaining_source() {
    let mut a = Local::subject::<i32, Infallible>();
    let done = Rc::new(std::cell::Cell::new(false));
    let c = done.clone();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    a.clone()
      .zip(Local::from_iter([10, 20]))
      .on_complete(move || c.set(true))
      .subscribe(move |x| v.borrow_mut().push(x));
    a.next(1);
    assert!(!done.get());
    a.next(2);
    assert!(done.get());
    use crate::scheduler::SleepProvider;
    crate::scheduler::LocalScheduler
      .sleep(Duration::from_millis(1))
      .await;
    assert_eq!(*values.borrow(), vec![(1, 10), (2, 20)]);
    assert_eq!(a.inner().subscriber_count(), 0);
  }
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
