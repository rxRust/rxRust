//! SwitchMap operator
//!
//! Transforms each value emitted by the source into an inner Observable, and
//! forwards items from only the most recently created inner Observable. When a
//! new inner Observable is produced, the previous one is unsubscribed.
//!
//! Behavior summary:
//! - Only the latest inner Observable's emissions are forwarded downstream.
//! - The operator completes only after the source completes and the current
//!   inner Observable completes.
//! - Errors from the source or from the current inner Observable are propagated
//!   immediately.
//!
//! Common uses: canceling in-flight operations (e.g. API calls) when new data
//! arrives, implementing type-ahead search, or switching between streams based
//! on user input.
//!
//! Example:
//!
//! ```rust no_run
//! use rxrust::prelude::*;
//!
//! let mut source = Local::subject();
//! let subscription = source
//!   .clone()
//!   .switch_map(|value| {
//!     Local::timer(Duration::from_millis(100)).map(move |_| format!("Result from {}", value))
//!   })
//!   .subscribe(|result| println!("{}", result));
//!
//! source.next(1);
//! source.next(2);
//! ```

use crate::{
  context::{Context, RcDeref, RcDerefMut, Scope},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// SwitchMap operator implementation.
///
/// This struct represents the SwitchMap operator that transforms each item from
/// a source Observable into a new Observable, then emits values from the most
/// recently created Observable.
///
/// # Type Parameters
///
/// * `S` - The source Observable type
/// * `F` - The transformation function that maps source items to inner
///   Observables
///
/// # Fields
///
/// * `source` - The source Observable that emits items to be transformed
/// * `func` - The closure/function that maps each source item to an inner
///   Observable
/// ```
#[derive(Clone)]
pub struct SwitchMap<S, F> {
  pub source: S,
  pub func: F,
}

#[doc(hidden)]
pub struct SwitchMapState<O, InnerSub> {
  observer: O,
  outer_completed: bool,
  inner_sub: Option<InnerSub>,
}

impl<O, InnerSub> Subscription for SwitchMapState<O, InnerSub>
where
  InnerSub: Subscription,
{
  fn unsubscribe(mut self) {
    if let Some(inner) = self.inner_sub.take() {
      inner.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool { false }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct SwitchMapOuterObserver<Sc: Scope, O, F> {
  state: SwitchState<Sc, O>,
  func: F,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct SwitchMapInnerObserver<State>(State);

/// Subscription type returned by the `switch_map` operator.
///
/// This subscription manages both the outer (source) subscription and the
/// current inner subscription. When unsubscribed, it will cancel both
/// subscriptions to ensure proper cleanup of resources.
///
/// # Type Parameters
///
/// * `SrcSub` - The subscription type for the source Observable
/// * `InnerSub` - The subscription type for managing the inner Observable state
///
/// # Behavior
///
/// - When `unsubscribe()` is called, both the outer and current inner
///   subscriptions are canceled
/// - The inner subscription can be None (when no inner Observable is active)
/// - Proper cleanup is guaranteed even if inner Observables are switched
///   rapidly
///
/// # Example
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut source = Local::subject();
/// let subscription = source
///   .switch_map(|x: i32| Local::of(x))
///   .subscribe(|_| {});
///
/// // The subscription type here is SwitchMapSubscription
/// // Unsubscribing will cancel both the source and any active inner subscriptions
/// subscription.unsubscribe();
/// ```
pub type SwitchMapSubscription<SrcSub, InnerSub> = TupleSubscription<SrcSub, InnerSub>;

impl<S, F, Out> ObservableType for SwitchMap<S, F>
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

type SwitchState<Sc, O> =
  <Sc as Scope>::RcMut<Option<SwitchMapState<O, <Sc as Scope>::BoxedSubscription>>>;
type InnerObserverCtx<C> = <C as Context>::With<
  SwitchMapInnerObserver<SwitchState<<C as Context>::Scope, <C as Context>::Inner>>,
>;

impl<S, F, C, Out, InnerObs> CoreObservable<C> for SwitchMap<S, F>
where
  C: Context,
  S: CoreObservable<C::With<SwitchMapOuterObserver<C::Scope, C::Inner, F>>>,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
  Out: Context<Inner = InnerObs>,
  InnerObs: CoreObservable<InnerObserverCtx<C>, Err = S::Err> + 'static,
  InnerObs::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SwitchState<C::Scope, C::Inner>: Subscription,
{
  type Unsub = SwitchMapSubscription<S::Unsub, SwitchState<C::Scope, C::Inner>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SwitchMap { source, func } = self;
    let state: SwitchState<C::Scope, C::Inner> = <C::Scope as Scope>::RcMut::from(None);

    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() =
        Some(SwitchMapState { observer, outer_completed: false, inner_sub: None });
      SwitchMapOuterObserver { state: state.clone(), func }
    });

    let source_unsub = source.subscribe(wrapped);

    TupleSubscription::new(source_unsub, state)
  }
}

impl<Sc, O, InnerObs, Item, Err, F, Out> Observer<Item, Err> for SwitchMapOuterObserver<Sc, O, F>
where
  Sc: Scope,
  O: for<'a> Observer<InnerObs::Item<'a>, Err>,
  F: for<'a> FnMut(Item) -> Out,
  Out: Context<Inner = InnerObs, Scope = Sc>,
  InnerObs: CoreObservable<
      Out::With<SwitchMapInnerObserver<SwitchState<Sc, O>>>,
      Unsub: IntoBoxedSubscription<Sc::BoxedSubscription>,
    >,
{
  fn next(&mut self, value: Item) {
    if self.is_closed() {
      return;
    }

    {
      let mut guard = self.state.rc_deref_mut();
      // Take the inner state before unsubscribing to avoid the shared state
      // being dropped while performing the unsubscribe.
      if let Some(mut st) = guard.take() {
        if let Some(prev) = st.inner_sub.take() {
          prev.unsubscribe();
        }
        *guard = Some(st);
      }
    }
    let inner_ctx = (self.func)(value);
    let inner_obs = inner_ctx.into_inner();

    let inner_observer = SwitchMapInnerObserver(self.state.clone());

    let inner_unsub = inner_obs.subscribe(Out::lift(inner_observer));
    if let Some(st) = self.state.rc_deref_mut().as_mut() {
      st.inner_sub = Some(inner_unsub.into_boxed());
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

    // If no active inner subscription, we can complete immediately.
    if st.inner_sub.is_none() {
      let st = guard.take().unwrap();
      st.observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|st| O::is_closed(&st.observer))
  }
}

impl<Item, Err, O, State, InnerSubState> Observer<Item, Err> for SwitchMapInnerObserver<State>
where
  O: Observer<Item, Err>,
  State: RcDerefMut<Target = Option<SwitchMapState<O, InnerSubState>>> + Clone,
{
  fn next(&mut self, value: Item) {
    if let Some(st) = self.0.rc_deref_mut().as_mut() {
      st.observer.next(value);
    }
  }

  fn error(self, err: Err) {
    if let Some(mut st) = self.0.rc_deref_mut().take() {
      let _ = st.inner_sub.take();
      st.observer.error(err);
    }
  }

  fn complete(self) {
    let mut guard = self.0.rc_deref_mut();
    let Some(st) = guard.as_mut() else { return };

    // Mark inner as completed.
    let _ = st.inner_sub.take();

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
      .is_none_or(|st| O::is_closed(&st.observer))
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_switch_map_only_latest_inner_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut outer = Local::subject::<i32, Infallible>();
    let mut inner1 = Local::subject::<&'static str, Infallible>();
    let mut inner2 = Local::subject::<&'static str, Infallible>();

    let inner1_for_map = inner1.clone();
    let inner2_for_map = inner2.clone();

    let _subscription = outer
      .clone()
      .switch_map(move |x| if x == 1 { inner1_for_map.clone() } else { inner2_for_map.clone() })
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    outer.next(1);
    inner1.next("a");

    outer.next(2); // switch to inner2
    inner1.next("b"); // ignored
    inner2.next("c");

    assert_eq!(*result.borrow(), vec!["a", "c"]);
  }

  #[rxrust_macro::test(local)]
  async fn test_switch_map_completion_waits_for_inner() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut outer = Local::subject::<i32, Infallible>();
    let inner = Local::subject::<i32, Infallible>();
    let inner_for_map = inner.clone();

    let _subscription = outer
      .clone()
      .switch_map(move |_| inner_for_map.clone())
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(|_| {});

    outer.next(1);
    outer.complete();
    assert!(!*completed.borrow());

    inner.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_switch_map_inner_error_errors_downstream() {
    let got_error = Rc::new(RefCell::new(false));
    let got_error_clone = got_error.clone();

    let mut outer = Local::subject::<(), &'static str>();

    let _subscription = outer
      .clone()
      .switch_map(|_| Local::throw_err("boom"))
      .on_error(move |_e| {
        *got_error_clone.borrow_mut() = true;
      })
      .subscribe(|_| {});

    outer.next(());
    assert!(*got_error.borrow());
  }
}
