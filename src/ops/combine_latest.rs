//! CombineLatest operator implementation
//!
//! CombineLatest combines the latest values from two observables using a binary
//! function. It emits whenever either source emits, using the most recent value
//! from the other source.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    IntoBoxedSubscription, SingleAssignment, Subscription, TupleSubscription,
    single_assignment::State,
  },
};

// ==================== CombineLatest Operator ====================

/// CombineLatest operator
///
/// Combines items from two observables using a binary function.
/// It tracks the latest value from each source and emits the result of the
/// binary function whenever either source emits, provided both sources have
/// emitted at least once.
#[derive(Clone)]
pub struct CombineLatest<A, B, F> {
  pub source_a: A,
  pub source_b: B,
  pub binary_op: F,
}

impl<A, B, F, OutputItem> ObservableType for CombineLatest<A, B, F>
where
  A: ObservableType,
  B: ObservableType<Err = A::Err>,
  F: FnMut(A::Item<'_>, B::Item<'_>) -> OutputItem,
{
  type Item<'a>
    = OutputItem
  where
    Self: 'a;
  type Err = A::Err;
}

// ==================== Shared State ====================

/// Shared state between A and B observers
pub struct CombineLatestState<O, ItemA, ItemB, F, E> {
  terminal: Option<Result<(), E>>,
  terminated: bool,
  observer: Option<O>,
  last_a: Option<ItemA>,
  last_b: Option<ItemB>,
  completed_a: bool,
  completed_b: bool,
  binary_op: F,
}

impl<O, ItemA, ItemB, F, Output, E> CombineLatestState<O, ItemA, ItemB, F, E>
where
  F: FnMut(ItemA, ItemB) -> Output,
{
  fn new(observer: O, binary_op: F) -> Self {
    Self {
      observer: Some(observer),
      terminal: None,
      terminated: false,
      last_a: None,
      last_b: None,
      completed_a: false,
      completed_b: false,
      binary_op,
    }
  }

  fn should_complete(&self) -> bool {
    (self.completed_a && self.last_a.is_none())
      || (self.completed_b && self.last_b.is_none())
      || (self.completed_a && self.completed_b)
  }
  fn finish(&mut self, terminal: Result<(), E>) -> Option<(O, Result<(), E>)> {
    if self.terminated {
      return None;
    }
    self.terminated = true;
    if let Some(observer) = self.observer.take() {
      Some((observer, terminal))
    } else {
      self.terminal = Some(terminal);
      None
    }
  }
  fn restore(&mut self, observer: O) -> Option<(O, Result<(), E>)> {
    if let Some(terminal) = self.terminal.take() {
      Some((observer, terminal))
    } else {
      self.observer = Some(observer);
      None
    }
  }
}

// ==================== Observer Structs ====================

/// Observer for source A
pub struct CombineLatestAObserver<StateRc, BProxy> {
  state: StateRc,
  b_proxy: BProxy,
}

/// Observer for source B
pub struct CombineLatestBObserver<StateRc, AProxy> {
  state: StateRc,
  a_proxy: AProxy,
}

// ==================== Type Aliases ====================

type SharedState<'a, C, A, B, F> = <C as Context>::RcMut<
  CombineLatestState<
    <C as Context>::Inner,
    <A as ObservableType>::Item<'a>,
    <B as ObservableType>::Item<'a>,
    F,
    <A as ObservableType>::Err,
  >,
>;

type SubProxy<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type BoxedSubProxy<C> =
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>;
type ClaCtx<'a, C, A, B, F, BUnsub> =
  <C as Context>::With<CombineLatestAObserver<SharedState<'a, C, A, B, F>, SubProxy<C, BUnsub>>>;
type ClbCtx<'a, C, A, B, F> =
  <C as Context>::With<CombineLatestBObserver<SharedState<'a, C, A, B, F>, BoxedSubProxy<C>>>;

// ==================== CoreObservable Implementation ====================

impl<A, B, F, C, AUnsub, BUnsub, OutputItem> CoreObservable<C> for CombineLatest<A, B, F>
where
  C: Context,
  A: for<'a> CoreObservable<ClaCtx<'a, C, A, B, F, BUnsub>, Unsub = AUnsub>,
  B: for<'a> CoreObservable<ClbCtx<'a, C, A, B, F>, Unsub = BUnsub, Err = A::Err>,
  F: FnMut(A::Item<'_>, B::Item<'_>) -> OutputItem + Clone,
  BUnsub: Subscription,
  AUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SubProxy<C, BUnsub>: Subscription,
  BoxedSubProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<BoxedSubProxy<C>, SubProxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let CombineLatest { source_a, source_b, binary_op } = self;

    let (downstream, scheduler) = context.into_parts();
    let state = C::RcMut::from(CombineLatestState::new(downstream, binary_op));

    let a_proxy: BoxedSubProxy<C> = SingleAssignment::new();

    // Subscribe B
    let b_observer = CombineLatestBObserver { state: state.clone(), a_proxy: a_proxy.clone() };
    let b_ctx = C::With::from_parts(b_observer, scheduler.clone());
    let b_unsub = source_b.subscribe(b_ctx);
    let b_proxy: SubProxy<C, BUnsub> = SingleAssignment::new();
    b_proxy.set(b_unsub);

    // Subscribe A
    if state.rc_deref().observer.is_some() {
      let a_observer = CombineLatestAObserver { state, b_proxy: b_proxy.clone() };
      let a_ctx = C::With::from_parts(a_observer, scheduler);
      let a_unsub = source_a.subscribe(a_ctx);
      a_proxy.set(a_unsub.into_boxed());
    }

    TupleSubscription::new(a_proxy, b_proxy)
  }
}

// ==================== Observer Implementations ====================

impl<ItemA, ItemB, Err, O, StateRc, BProxy, F, OutputItem> Observer<ItemA, Err>
  for CombineLatestAObserver<StateRc, BProxy>
where
  StateRc: RcDerefMut<Target = CombineLatestState<O, ItemA, ItemB, F, Err>>,
  BProxy: Subscription,
  O: Observer<OutputItem, Err>,
  F: FnMut(ItemA, ItemB) -> OutputItem,
  ItemA: Clone,
  ItemB: Clone,
{
  fn next(&mut self, value: ItemA) {
    let (output, observer) = {
      let mut state = self.state.rc_deref_mut();
      if state.terminated {
        return;
      }
      state.last_a = Some(value.clone());
      let output = state
        .last_b
        .clone()
        .map(|other| (state.binary_op)(value, other));
      (output, state.observer.take())
    };
    if let Some(mut observer) = observer {
      if let Some(output) = output {
        observer.next(output);
      }
      let terminal = { self.state.rc_deref_mut().restore(observer) };
      if let Some((observer, terminal)) = terminal {
        match terminal {
          Ok(()) => observer.complete(),
          Err(e) => observer.error(e),
        }
      }
    }
  }
  fn error(self, err: Err) {
    let (terminal, cancel_peer) = {
      let mut state = self.state.rc_deref_mut();
      let cancel_peer = !state.terminated && !state.completed_b;
      (state.finish(Err(err)), cancel_peer)
    };
    if cancel_peer {
      self.b_proxy.unsubscribe();
    }
    if let Some((observer, Err(err))) = terminal {
      observer.error(err);
    }
  }
  fn complete(self) {
    let (terminal, cancel_peer) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_a = true;
      if state.should_complete() && !state.terminated {
        let cancel_peer = !state.completed_b;
        (state.finish(Ok(())), cancel_peer)
      } else {
        (None, false)
      }
    };
    if cancel_peer {
      self.b_proxy.unsubscribe();
    }
    if let Some((observer, Ok(()))) = terminal {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

impl<ItemA, ItemB, Err, O, StateRc, AProxy, F, OutputItem> Observer<ItemB, Err>
  for CombineLatestBObserver<StateRc, AProxy>
where
  StateRc: RcDerefMut<Target = CombineLatestState<O, ItemA, ItemB, F, Err>>,
  AProxy: Subscription,
  O: Observer<OutputItem, Err>,
  F: FnMut(ItemA, ItemB) -> OutputItem,
  ItemA: Clone,
  ItemB: Clone,
{
  fn next(&mut self, value: ItemB) {
    let (output, observer) = {
      let mut state = self.state.rc_deref_mut();
      if state.terminated {
        return;
      }
      state.last_b = Some(value.clone());
      let output = state
        .last_a
        .clone()
        .map(|other| (state.binary_op)(other, value));
      (output, state.observer.take())
    };
    if let Some(mut observer) = observer {
      if let Some(output) = output {
        observer.next(output);
      }
      let terminal = { self.state.rc_deref_mut().restore(observer) };
      if let Some((observer, terminal)) = terminal {
        match terminal {
          Ok(()) => observer.complete(),
          Err(e) => observer.error(e),
        }
      }
    }
  }
  fn error(self, err: Err) {
    let (terminal, cancel_peer) = {
      let mut state = self.state.rc_deref_mut();
      let cancel_peer = !state.terminated && !state.completed_a;
      (state.finish(Err(err)), cancel_peer)
    };
    if cancel_peer {
      self.a_proxy.unsubscribe();
    }
    if let Some((observer, Err(err))) = terminal {
      observer.error(err);
    }
  }
  fn complete(self) {
    let (terminal, cancel_peer) = {
      let mut state = self.state.rc_deref_mut();
      state.completed_b = true;
      if state.should_complete() && !state.terminated {
        let cancel_peer = !state.completed_a;
        (state.finish(Ok(())), cancel_peer)
      } else {
        (None, false)
      }
    };
    if cancel_peer {
      self.a_proxy.unsubscribe();
    }
    if let Some((observer, Ok(()))) = terminal {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  #[rxrust_macro::test(local)]
  async fn reentrant_error_is_delivered_after_in_flight_next() {
    use std::{cell::Cell, rc::Rc};
    let mut a = Local::subject::<i32, &str>();
    let mut b = Local::subject::<i32, &str>();
    let fail = a.clone();
    let errors = Rc::new(Cell::new(0));
    let e = errors.clone();
    let values = Rc::new(Cell::new(0));
    let v = values.clone();
    a.clone()
      .combine_latest(b.clone(), |a, b| a + b)
      .on_error(move |_| e.set(e.get() + 1))
      .subscribe(move |_| {
        v.set(v.get() + 1);
        fail.clone().error("failed during next");
      });
    a.next(1);
    b.next(2);
    b.next(3);
    assert_eq!(values.get(), 1);
    assert_eq!(errors.get(), 1);
  }

  #[rxrust_macro::test(local)]
  async fn empty_side_completes_and_cancels_active_peer() {
    let a = Local::subject::<i32, std::convert::Infallible>();
    let b = Local::subject::<i32, std::convert::Infallible>();
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let c = done.clone();
    a.clone()
      .combine_latest(b.clone(), |a, b| a + b)
      .on_complete(move || c.set(true))
      .subscribe(|_| {});
    a.complete();
    assert!(done.get());
    assert_eq!(b.inner().subscriber_count(), 0);
  }
  use std::{
    sync::{Arc, Mutex},
    time::Duration,
  };

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_combine_latest_basic() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_clone = result.clone();

    // A: 0, 1, 2 every 20ms
    let source_a = Local::interval(Duration::from_millis(20)).take(3);
    // B: 0, 1 every 30ms
    let source_b = Local::interval(Duration::from_millis(30)).take(2);

    source_a
      .combine_latest(source_b, |a, b| (a, b))
      .subscribe(move |v| {
        result_clone.lock().unwrap().push(v);
      });

    // Wait enough time
    LocalScheduler
      .sleep(Duration::from_millis(100))
      .await;

    let res = result.lock().unwrap().clone();

    // Check if we got expected combinations
    assert!(!res.is_empty());
    assert!(res.contains(&(2, 1))); // Final values must appear
  }

  #[rxrust_macro::test(local)]
  async fn test_combine_latest_transform() {
    let sum = Arc::new(Mutex::new(0));
    let sum_clone = sum.clone();

    let a = Local::from_iter(vec![1, 2]);
    let b = Local::from_iter(vec![10, 20]);

    a.combine_latest(b, |x, y| x + y)
      .subscribe(move |v| *sum_clone.lock().unwrap() += v);

    assert_eq!(*sum.lock().unwrap(), 43);
  }

  #[rxrust_macro::test(local)]
  async fn test_combine_latest_complete() {
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = completed.clone();

    let mut subject_a = Local::subject::<i32, std::convert::Infallible>();
    let mut subject_b = Local::subject::<i32, std::convert::Infallible>();

    subject_a
      .clone()
      .combine_latest(subject_b.clone(), |a, b| (a, b))
      .on_complete(move || *completed_clone.lock().unwrap() = true)
      .subscribe(|_| {});

    subject_a.next(1);
    subject_b.next(2); // Emits (1,2)

    subject_a.complete();
    assert!(!*completed.lock().unwrap()); // Should not be complete yet

    subject_b.complete();
    assert!(*completed.lock().unwrap()); // Now complete
  }
}
