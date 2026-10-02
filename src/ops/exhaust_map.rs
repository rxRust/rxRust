//! ExhaustMap operator implementation
//!
//! Maps to inner observables but ignores outer items while an inner one is
//! active.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDeref, RcDerefMut, Scope},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// ExhaustMap operator: Run one inner observable at a time, dropping outer
/// items that arrive while one is active
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter(vec![1, 2, 3])
///   .exhaust_map(|v| Local::from_iter(vec![v * 10, v * 10 + 1]))
///   .subscribe(|v| result.push(v));
/// // Synchronous inners finish before the next outer item, so all run
/// assert_eq!(result, vec![10, 11, 20, 21, 30, 31]);
/// ```
#[doc(alias = "exhaustMap")]
#[derive(Clone)]
pub struct ExhaustMap<S, F> {
  pub source: S,
  pub func: F,
}

#[doc(hidden)]
pub struct ExhaustMapState<O, InnerSub> {
  observer: O,
  outer_completed: bool,
  inner_active: bool,
  inner_sub: Option<InnerSub>,
}

impl<O, InnerSub: Subscription> Subscription for ExhaustMapState<O, InnerSub> {
  fn unsubscribe(mut self) {
    if let Some(inner) = self.inner_sub.take() {
      inner.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool { false }
}

#[doc(hidden)]
pub struct ExhaustMapOuterObserver<Sc: Scope, O, F, InnerObs> {
  state: ExhaustState<Sc, O>,
  func: F,
  _inner: PhantomData<fn() -> InnerObs>,
}

#[doc(hidden)]
pub struct ExhaustMapInnerObserver<State>(State);

type ExhaustState<Sc, O> =
  <Sc as Scope>::RcMut<Option<ExhaustMapState<O, <Sc as Scope>::BoxedSubscription>>>;
type InnerObserverCtx<C> = <C as Context>::With<
  ExhaustMapInnerObserver<ExhaustState<<C as Context>::Scope, <C as Context>::Inner>>,
>;

impl<S, F, Out> ObservableType for ExhaustMap<S, F>
where
  S: ObservableType,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
  Out: Context<Inner: ObservableType<Err = S::Err> + 'static>,
{
  type Item<'a>
    = <Out::Inner as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, C, Out, InnerObs> CoreObservable<C> for ExhaustMap<S, F>
where
  C: Context,
  S: CoreObservable<C::With<ExhaustMapOuterObserver<C::Scope, C::Inner, F, InnerObs>>>,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
  Out: Context<Inner = InnerObs>,
  InnerObs: CoreObservable<InnerObserverCtx<C>, Err = S::Err> + 'static,
  InnerObs::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  ExhaustState<C::Scope, C::Inner>: Subscription,
{
  type Unsub = TupleSubscription<S::Unsub, ExhaustState<C::Scope, C::Inner>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let ExhaustMap { source, func } = self;
    let state: ExhaustState<C::Scope, C::Inner> = <C::Scope as Scope>::RcMut::from(None);

    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() = Some(ExhaustMapState {
        observer,
        outer_completed: false,
        inner_active: false,
        inner_sub: None,
      });
      ExhaustMapOuterObserver { state: state.clone(), func, _inner: PhantomData }
    });

    let source_unsub = source.subscribe(wrapped);
    TupleSubscription::new(source_unsub, state)
  }
}

impl<Sc, O, InnerObs, Item, Err, F, Out> Observer<Item, Err>
  for ExhaustMapOuterObserver<Sc, O, F, InnerObs>
where
  Sc: Scope,
  O: for<'a> Observer<InnerObs::Item<'a>, Err>,
  F: FnMut(Item) -> Out,
  Out: Context<Inner = InnerObs, Scope = Sc>,
  InnerObs: CoreObservable<
      Out::With<ExhaustMapInnerObserver<ExhaustState<Sc, O>>>,
      Unsub: IntoBoxedSubscription<Sc::BoxedSubscription>,
    >,
{
  fn next(&mut self, value: Item) {
    {
      let mut guard = self.state.rc_deref_mut();
      let Some(st) = guard.as_mut() else { return };
      if st.inner_active || st.observer.is_closed() {
        return;
      }
      st.inner_active = true;
    }
    let inner_obs = (self.func)(value).into_inner();
    let inner_unsub = inner_obs.subscribe(Out::lift(ExhaustMapInnerObserver(self.state.clone())));
    let mut guard = self.state.rc_deref_mut();
    if let Some(st) = guard.as_mut() {
      if st.inner_active {
        st.inner_sub = Some(inner_unsub.into_boxed());
      } else {
        // The inner finished synchronously; nothing to keep.
        drop(inner_unsub);
      }
    }
  }

  fn error(self, err: Err) {
    if let Some(mut st) = self.state.rc_deref_mut().take() {
      st.observer.error(err);
      if let Some(inner) = st.inner_sub.take() {
        inner.unsubscribe();
      }
    }
  }

  fn complete(self) {
    let mut guard = self.state.rc_deref_mut();
    let Some(st) = guard.as_mut() else { return };
    st.outer_completed = true;
    if !st.inner_active {
      let st = guard.take().unwrap();
      st.observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|st| st.observer.is_closed())
  }
}

impl<State, O, InnerSub, Item, Err> Observer<Item, Err> for ExhaustMapInnerObserver<State>
where
  State: RcDerefMut<Target = Option<ExhaustMapState<O, InnerSub>>>,
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    if let Some(st) = self.0.rc_deref_mut().as_mut() {
      st.observer.next(value);
    }
  }

  fn error(self, err: Err) {
    if let Some(st) = self.0.rc_deref_mut().take() {
      st.observer.error(err);
    }
  }

  fn complete(self) {
    let mut guard = self.0.rc_deref_mut();
    let Some(st) = guard.as_mut() else { return };
    st.inner_active = false;
    st.inner_sub = None;
    if st.outer_completed {
      let st = guard.take().unwrap();
      st.observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .0
      .rc_deref()
      .as_ref()
      .is_none_or(|st| st.observer.is_closed())
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_exhaust_map_drops_outer_items_while_inner_active() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut outer = Local::subject::<i32, Infallible>();
    let mut inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    outer
      .clone()
      .exhaust_map(move |_| inner_c.clone())
      .subscribe(move |v| result_c.borrow_mut().push(v));

    outer.next(1);
    inner.next(10);
    outer.next(2); // dropped: inner still active
    inner.next(11);
    inner.clone().complete();
    outer.next(3); // starts a new inner subscription
    assert_eq!(inner.inner.subscriber_count(), 1);
    inner.next(12);

    assert_eq!(*result.borrow(), vec![10, 11, 12]);
  }

  #[rxrust_macro::test]
  fn test_exhaust_map_sync_inners_all_run() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(vec![1, 2, 3])
      .exhaust_map(|v| Local::from_iter(vec![v * 10, v * 10 + 1]))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![10, 11, 20, 21, 30, 31]);
  }

  #[rxrust_macro::test]
  fn test_exhaust_map_completes_after_inner() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    let mut outer = Local::subject::<i32, Infallible>();
    let inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    outer
      .clone()
      .exhaust_map(move |_| inner_c.clone())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    outer.next(1);
    outer.clone().complete();
    assert!(!*completed.borrow());
    inner.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_exhaust_map_error_propagation() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    let outer = Local::subject::<i32, String>();
    outer
      .clone()
      .exhaust_map(|v| Local::of(v).map_err(|_: Infallible| String::new()))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    outer.error("boom".to_string());
    assert_eq!(error.borrow().as_deref(), Some("boom"));
  }

  #[rxrust_macro::test]
  fn test_exhaust_map_unsubscribe_cancels_inner() {
    let mut outer = Local::subject::<i32, Infallible>();
    let inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    let sub = outer
      .clone()
      .exhaust_map(move |_| inner_c.clone())
      .subscribe(|_| {});
    outer.next(1);
    assert_eq!(inner.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert_eq!(inner.inner.subscriber_count(), 0);
  }
}
