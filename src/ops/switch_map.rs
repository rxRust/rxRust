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

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDeref, RcDerefMut, Scope},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    IntoBoxedSubscription, SingleAssignment, Subscription, TupleSubscription,
    single_assignment::State,
  },
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

pub struct SwitchMapState<O, H> {
  observer: O,
  outer_completed: bool,
  inner_sub: Option<H>,
  generation: usize,
}
impl<O, H: Subscription> Subscription for SwitchMapState<O, H> {
  fn unsubscribe(self) { self.inner_sub.unsubscribe(); }
  fn is_closed(&self) -> bool { false }
}
type InnerSlot<Sc> =
  SingleAssignment<<Sc as Scope>::RcMut<State<<Sc as Scope>::BoxedSubscription>>>;
type SwitchState<Sc, O> =
  <Sc as Scope>::RcMut<Option<SwitchMapState<<Sc as Scope>::RcMut<Option<O>>, InnerSlot<Sc>>>>;
type SourceSlot<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
pub struct SwitchMapOuterObserver<Sc: Scope, O, F, I, H> {
  state: SwitchState<Sc, O>,
  func: F,
  outer: H,
  _inner: PhantomData<fn() -> I>,
}
pub struct SwitchMapInnerObserver<P, H> {
  state: P,
  outer: H,
  generation: usize,
}
pub type SwitchMapSubscription<U, P> = TupleSubscription<U, P>;
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
impl<S, F, C, Out, I, U> CoreObservable<C> for SwitchMap<S, F>
where
  C: Context,
  U: Subscription,
  S: CoreObservable<C::With<SwitchMapOuterObserver<C::Scope, C::Inner, F, I, ()>>, Unsub = U>
    + CoreObservable<
      C::With<SwitchMapOuterObserver<C::Scope, C::Inner, F, I, SourceSlot<C, U>>>,
      Unsub = U,
    >,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
  Out: Context<Inner = I>,
  I: ObservableType<Err = S::Err> + 'static,
  SwitchState<C::Scope, C::Inner>: Subscription,
{
  type Unsub = SwitchMapSubscription<SourceSlot<C, U>, SwitchState<C::Scope, C::Inner>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let state = <C::Scope as Scope>::RcMut::from(None);
    let outer = SourceSlot::<C, U>::new();
    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() = Some(SwitchMapState {
        observer: <C::Scope as Scope>::RcMut::from(Some(observer)),
        outer_completed: false,
        inner_sub: None,
        generation: 0,
      });
      SwitchMapOuterObserver {
        state: state.clone(),
        func: self.func,
        outer: outer.clone(),
        _inner: PhantomData,
      }
    });
    outer.set(self.source.subscribe(wrapped));
    TupleSubscription::new(outer, state)
  }
}
impl<Sc, O, I, V, E, F, Out, H> Observer<V, E> for SwitchMapOuterObserver<Sc, O, F, I, H>
where
  Sc: Scope,
  O: for<'a> Observer<I::Item<'a>, E>,
  F: FnMut(V) -> Out,
  H: Subscription + Clone,
  Out: Context<Inner = I, Scope = Sc>,
  I: CoreObservable<
      Out::With<SwitchMapInnerObserver<SwitchState<Sc, O>, H>>,
      Unsub: IntoBoxedSubscription<Sc::BoxedSubscription>,
    >,
{
  fn next(&mut self, v: V) {
    let slot = InnerSlot::<Sc>::new();
    let (previous, generation) = {
      let mut guard = self.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else {
        return;
      };
      state.generation += 1;
      (state.inner_sub.replace(slot.clone()), state.generation)
    };
    previous.unsubscribe();
    if self
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|s| s.generation != generation)
    {
      return;
    }
    let (core, ctx) = (self.func)(v).swap(SwitchMapInnerObserver {
      state: self.state.clone(),
      outer: self.outer.clone(),
      generation,
    });
    slot.set(core.subscribe(ctx).into_boxed());
  }
  fn error(self, e: E) {
    let state = { self.state.rc_deref_mut().take() };
    if let Some(state) = state {
      state.inner_sub.unsubscribe();
      let observer = { state.observer.rc_deref_mut().take() };
      if let Some(observer) = observer {
        observer.error(e);
      }
    }
  }
  fn complete(self) {
    let state = {
      let mut guard = self.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else {
        return;
      };
      state.outer_completed = true;
      if state.inner_sub.is_none() { guard.take() } else { None }
    };
    if let Some(state) = state {
      let observer = { state.observer.rc_deref_mut().take() };
      if let Some(observer) = observer {
        observer.complete();
      }
    }
  }
  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|s| s.observer.rc_deref().is_closed())
  }
}
impl<V, E, O, OP, P, H, W> Observer<V, E> for SwitchMapInnerObserver<P, H>
where
  O: Observer<V, E>,
  OP: RcDerefMut<Target = Option<O>>,
  P: RcDerefMut<Target = Option<SwitchMapState<OP, W>>>,
  H: Subscription,
{
  fn next(&mut self, v: V) {
    let observer = {
      self
        .state
        .rc_deref()
        .as_ref()
        .filter(|s| s.generation == self.generation && s.inner_sub.is_some())
        .map(|s| s.observer.clone())
    };
    if let Some(observer) = observer
      && let Some(o) = observer.rc_deref_mut().as_mut()
    {
      o.next(v);
    }
  }
  fn error(self, e: E) {
    let state = {
      let mut guard = self.state.rc_deref_mut();
      if guard
        .as_ref()
        .is_some_and(|s| s.generation == self.generation && s.inner_sub.is_some())
      {
        guard.take()
      } else {
        None
      }
    };
    if let Some(state) = state {
      if !state.outer_completed {
        self.outer.unsubscribe();
      }
      let observer = { state.observer.rc_deref_mut().take() };
      if let Some(observer) = observer {
        observer.error(e);
      }
    }
  }
  fn complete(self) {
    let (retired, state) = {
      let mut guard = self.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else {
        return;
      };
      if state.generation != self.generation {
        return;
      }
      let retired = state.inner_sub.take();
      let state = if state.outer_completed { guard.take() } else { None };
      (retired, state)
    };
    drop(retired);
    if let Some(state) = state {
      let observer = { state.observer.rc_deref_mut().take() };
      if let Some(observer) = observer {
        observer.complete();
      }
    }
  }
  fn is_closed(&self) -> bool {
    self.state.rc_deref().as_ref().is_none_or(|s| {
      s.generation != self.generation || s.inner_sub.is_none() || s.observer.rc_deref().is_closed()
    })
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn old_generation_notifications_cannot_finish_new_inner() {
    use crate::test_support::Manual;
    let outer = Manual::default();
    let inner = Manual::default();
    let i = inner.clone();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let done = Rc::new(std::cell::Cell::new(false));
    let d = done.clone();
    Local::new(outer.clone())
      .switch_map(move |_| Local::new(i.clone()))
      .on_error(|_| {})
      .on_complete(move || d.set(true))
      .subscribe(move |x| v.borrow_mut().push(x));
    outer.next(0, 1);
    outer.next(0, 2);
    inner.next(0, 99);
    inner.complete(0);
    inner.next(1, 2);
    outer.complete(0);
    assert!(!done.get());
    inner.complete(1);
    assert!(done.get());
    assert_eq!(*values.borrow(), vec![2]);
    assert_eq!(inner.cancellations(0), 1);
    assert_eq!(inner.cancellations(1), 0);
  }
  #[rxrust_macro::test]
  fn synchronous_inner_completion_retires_late_handle() {
    use crate::subscription::ClosureSubscription;
    let count = Rc::new(std::cell::Cell::new(0));
    let c = count.clone();
    let done = Rc::new(std::cell::Cell::new(false));
    let d = done.clone();
    let sub = Local::of(1)
      .switch_map(move |_| {
        let c = c.clone();
        Local::create(move |e| {
          e.next(1);
          e.complete();
          ClosureSubscription(move || c.set(c.get() + 1))
        })
      })
      .on_complete(move || d.set(true))
      .subscribe(|_| {});
    assert!(done.get());
    sub.unsubscribe();
    assert_eq!(count.get(), 0);
  }
  #[rxrust_macro::test]
  fn cancellation_inside_inner_callback_is_reentrant() {
    use crate::{subscription::BoxedSubscription, test_support::Manual};
    let source = Manual::default();
    let inner = Manual::default();
    let i = inner.clone();
    let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
    let h = holder.clone();
    let sub = Local::new(source.clone())
      .switch_map(move |_| Local::new(i.clone()))
      .on_error(|_| {})
      .subscribe(move |_| h.borrow_mut().take().unwrap().unsubscribe());
    *holder.borrow_mut() = Some(BoxedSubscription::new(sub));
    source.next(0, 1);
    inner.next(0, 1);
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(inner.cancellations(0), 1);
  }
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
