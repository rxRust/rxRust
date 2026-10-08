//! WithLatestFrom operator implementation
//!
//! Combines each item from the source observable with the latest value from
//! another observable. Only emits when the primary source emits, not when the
//! secondary emits.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    IntoBoxedSubscription, SingleAssignment, Subscription, TupleSubscription,
    single_assignment::State,
  },
};

// ==================== WithLatestFrom Operator ====================

/// WithLatestFrom operator
///
/// Combines each item from source A with the latest value from source B.
/// Only emits when A emits. If B hasn't emitted yet, A's emissions are dropped.
#[derive(Clone)]
pub struct WithLatestFrom<A, B> {
  pub source_a: A,
  pub source_b: B,
}

impl<A, B> ObservableType for WithLatestFrom<A, B>
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
pub struct WithLatestFromState<O, ItemB> {
  observer: Option<O>,
  last_b: Option<ItemB>,
  completed_b: bool,
}

impl<O, ItemB> WithLatestFromState<O, ItemB> {
  fn new(observer: O) -> Self {
    Self { observer: Some(observer), last_b: None, completed_b: false }
  }
}

// ==================== Observer Structs ====================

/// Observer for source A (primary - drives emissions)
pub struct WithLatestFromAObserver<StateRc, BProxy> {
  state: StateRc,
  b_proxy: BProxy,
}

/// Observer for source B (secondary - only stores value)
/// ItemA is included via PhantomData to make type checking work correctly
pub struct WithLatestFromBObserver<StateRc, AProxy, ItemA> {
  state: StateRc,
  a_proxy: AProxy,
  _marker: PhantomData<fn() -> ItemA>,
}

// ==================== Type Aliases ====================

type SharedState<'a, C, B> = <C as Context>::RcMut<
  WithLatestFromState<<C as Context>::Inner, <B as ObservableType>::Item<'a>>,
>;

type SubProxy<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type BoxedSubProxy<C> =
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>;

type WlfACtx<'a, C, B, BUnsub> =
  <C as Context>::With<WithLatestFromAObserver<SharedState<'a, C, B>, SubProxy<C, BUnsub>>>;
type WlfBCtx<'a, C, A, B> = <C as Context>::With<
  WithLatestFromBObserver<SharedState<'a, C, B>, BoxedSubProxy<C>, <A as ObservableType>::Item<'a>>,
>;

// ==================== CoreObservable Implementation ====================

impl<A, B, C, AUnsub, BUnsub> CoreObservable<C> for WithLatestFrom<A, B>
where
  C: Context,
  A: for<'a> CoreObservable<WlfACtx<'a, C, B, BUnsub>, Unsub = AUnsub>,
  B: for<'a> CoreObservable<WlfBCtx<'a, C, A, B>, Unsub = BUnsub, Err = A::Err>,
  BUnsub: Subscription,
  AUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SubProxy<C, BUnsub>: Subscription,
  BoxedSubProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<BoxedSubProxy<C>, SubProxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let WithLatestFrom { source_a, source_b } = self;

    let (downstream, scheduler) = context.into_parts();
    let state = C::RcMut::from(WithLatestFromState::new(downstream));

    let (install_a, [a_proxy, a_subscription]) = BoxedSubProxy::<C>::channel();

    // Subscribe B first
    let b_observer =
      WithLatestFromBObserver { state: state.clone(), a_proxy, _marker: PhantomData };
    let b_ctx = C::With::from_parts(b_observer, scheduler.clone());
    let b_unsub = source_b.subscribe(b_ctx);
    let (install_b, [b_proxy, b_subscription]) = SubProxy::<C, BUnsub>::channel();
    install_b(b_unsub);

    // Subscribe A
    if state.rc_deref().observer.is_some() {
      let a_observer = WithLatestFromAObserver { state, b_proxy };
      let a_ctx = C::With::from_parts(a_observer, scheduler);
      let a_unsub = source_a.subscribe(a_ctx);
      install_a(a_unsub.into_boxed());
    }

    TupleSubscription::new(a_subscription, b_subscription)
  }
}

// ==================== Observer Implementations ====================

impl<ItemA, ItemB, Err, O, StateRc, BProxy> Observer<ItemA, Err>
  for WithLatestFromAObserver<StateRc, BProxy>
where
  StateRc: RcDerefMut<Target = WithLatestFromState<O, ItemB>>,
  BProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
  ItemB: Clone,
{
  fn next(&mut self, value: ItemA) {
    let mut guard = self.state.rc_deref_mut();
    if let Some(b) = guard.last_b.clone()
      && let Some(observer) = guard.observer.as_mut()
    {
      observer.next((value, b));
    }
    // If no last_b, drop the value (as per WithLatestFrom semantics)
  }

  fn error(self, err: Err) {
    let (observer, completed_b) = {
      let mut state = self.state.rc_deref_mut();
      (state.observer.take(), state.completed_b)
    };
    if let Some(observer) = observer {
      if !completed_b {
        self.b_proxy.unsubscribe();
      }
      observer.error(err);
    }
  }

  fn complete(self) {
    let (observer, completed_b) = {
      let mut state = self.state.rc_deref_mut();
      (state.observer.take(), state.completed_b)
    };
    if let Some(observer) = observer {
      if !completed_b {
        self.b_proxy.unsubscribe();
      }
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

// BObserver - ItemA is constrained via PhantomData in the struct
impl<ItemA, ItemB, Err, O, StateRc, AProxy> Observer<ItemB, Err>
  for WithLatestFromBObserver<StateRc, AProxy, ItemA>
where
  StateRc: RcDerefMut<Target = WithLatestFromState<O, ItemB>>,
  AProxy: Subscription,
  O: Observer<(ItemA, ItemB), Err>,
{
  fn next(&mut self, value: ItemB) { self.state.rc_deref_mut().last_b = Some(value); }

  fn error(self, err: Err) {
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      self.a_proxy.unsubscribe();
      observer.error(err);
    }
  }

  fn complete(self) {
    self.state.rc_deref_mut().completed_b = true;
    // Secondary completion does NOT complete downstream
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {

  #[rxrust_macro::test(local)]
  async fn secondary_completion_keeps_value_without_cancelling_it() {
    use crate::subscription::ClosureSubscription;
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = calls.clone();
    let b = Local::create(move |e| {
      e.next(10);
      e.complete();
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    let mut values = vec![];
    Local::from_iter([1, 2])
      .with_latest_from(b)
      .subscribe(|v| values.push(v));
    assert_eq!(values, vec![(1, 10), (2, 10)]);
    assert_eq!(calls.get(), 0);
  }
  use std::sync::{Arc, Mutex};

  use crate::prelude::*;

  #[rxrust_macro::test]
  async fn test_simple() {
    let ret = Arc::new(Mutex::new(String::new()));
    let ret_clone = ret.clone();

    let mut s1 = Local::subject::<char, std::convert::Infallible>();
    let mut s2 = Local::subject::<char, std::convert::Infallible>();

    s1.clone()
      .with_latest_from(s2.clone())
      .subscribe(move |(a, b)| {
        let mut guard = ret_clone.lock().unwrap();
        guard.push(a);
        guard.push(b);
      });

    s1.next('1');
    s2.next('A');
    s1.next('2'); // 2A
    s2.next('B');
    s2.next('C');
    s2.next('D');
    s1.next('3'); // 3D
    s1.next('4'); // 4D
    s1.next('5'); // 5D

    assert_eq!(*ret.lock().unwrap(), "2A3D4D5D");
  }

  #[rxrust_macro::test]
  async fn test_smoke() {
    let a_store = Arc::new(Mutex::new(vec![]));
    let b_store = Arc::new(Mutex::new(vec![]));
    let numbers_store = Arc::new(Mutex::new(vec![]));

    let a_store_clone = a_store.clone();
    let b_store_clone = b_store.clone();
    let numbers_store_clone = numbers_store.clone();

    let mut numbers = Local::subject::<i32, std::convert::Infallible>();

    numbers
      .clone()
      .filter(|v| *v % 3 == 0)
      .with_latest_from(numbers.clone().filter(|v| *v % 3 != 0))
      .subscribe(move |v| numbers_store_clone.lock().unwrap().push(v));

    numbers
      .clone()
      .filter(|v| *v % 3 == 0)
      .subscribe(move |v| a_store_clone.lock().unwrap().push(v));

    numbers
      .clone()
      .filter(|v| *v % 3 != 0)
      .subscribe(move |v| b_store_clone.lock().unwrap().push(v));

    (0..10).for_each(|v| {
      numbers.next(v);
    });

    assert_eq!(*a_store.lock().unwrap(), vec![0, 3, 6, 9]);
    assert_eq!(*b_store.lock().unwrap(), vec![1, 2, 4, 5, 7, 8]);
    assert_eq!(*numbers_store.lock().unwrap(), vec![(3, 2), (6, 5), (9, 8)]);
  }

  #[rxrust_macro::test]
  async fn test_primary_complete() {
    let complete = Arc::new(Mutex::new(false));
    let complete_clone = complete.clone();

    let s1 = Local::subject::<(), std::convert::Infallible>();
    s1.clone()
      .with_latest_from(Local::subject::<(), std::convert::Infallible>())
      .on_complete(move || *complete_clone.lock().unwrap() = true)
      .subscribe(|(_a, _b)| {});

    s1.complete();

    assert!(*complete.lock().unwrap());
  }

  #[rxrust_macro::test]
  async fn test_secondary_complete() {
    let complete = Arc::new(Mutex::new(false));
    let complete_clone = complete.clone();

    let s1 = Local::subject::<(), std::convert::Infallible>();
    let s2 = Local::subject::<(), std::convert::Infallible>();
    s1.clone()
      .with_latest_from(s2.clone())
      .on_complete(move || *complete_clone.lock().unwrap() = true)
      .subscribe(|(_a, _b)| {});

    s2.complete();

    assert!(!*complete.lock().unwrap());
  }
}
