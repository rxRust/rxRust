//! Expand operator implementation
//!
//! Recursively projects every emitted item into an observable and merges
//! the results.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// Expand operator: Recursive `flat_map`
///
/// Emits every source item, then feeds each emitted item (from the source or
/// from an inner observable) to `f` and merges the results, recursively.
/// Items must be `Clone`. Completes when the source and every inner
/// observable have completed; an inner that never completes keeps the
/// expansion alive, and unbounded recursion must be cut with an empty inner.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::of(1)
///   .expand(|v| if v < 8 { Local::from_iter(vec![v * 2]) } else { Local::from_iter(vec![]) })
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 4, 8]);
/// ```
#[derive(Clone)]
pub struct Expand<S, F> {
  pub source: S,
  pub func: F,
}

impl<S, F> ObservableType for Expand<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// State shared by every observer of the expansion
pub struct ExpandState<O> {
  observer: Option<O>,
  active: usize,
  outer_done: bool,
}

impl<O> ExpandState<O> {
  fn complete_if_done<Item, Err>(&mut self)
  where
    O: Observer<Item, Err>,
  {
    if self.outer_done
      && self.active == 0
      && let Some(observer) = self.observer.take()
    {
      observer.complete();
    }
  }
}

/// Observer for the source and for every inner observable
///
/// `spawn` is a function pointer instantiated where the inner observable's
/// bounds are known, so this observer's own bounds stay non-recursive.
pub struct ExpandObserver<StateRc, SubsRc, FuncRc, Item> {
  state: StateRc,
  subs: SubsRc,
  func: FuncRc,
  /// `None` for the source observer, `Some(id)` for an inner one
  id: Option<usize>,
  spawn: fn(&StateRc, &SubsRc, &FuncRc, Item),
}

/// Emits `value` downstream and subscribes its expansion.
fn spawn_expansion<StateRc, SubsRc, FuncRc, O, F, Item, Err, Out, U>(
  state: &StateRc, subs: &SubsRc, func: &FuncRc, value: Item,
) where
  StateRc: RcDerefMut<Target = ExpandState<O>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  FuncRc: RcDerefMut<Target = F> + Clone,
  O: Observer<Item, Err>,
  F: FnMut(Item) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<ExpandObserver<StateRc, SubsRc, FuncRc, Item>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
  Item: Clone,
{
  {
    let mut st = state.rc_deref_mut();
    let Some(observer) = st.observer.as_mut() else { return };
    observer.next(value.clone());
    st.active += 1;
  }
  let inner = (func.rc_deref_mut())(value).into_inner();
  let id = subs.rc_deref_mut().reserve_id();
  let observer = ExpandObserver {
    state: state.clone(),
    subs: subs.clone(),
    func: func.clone(),
    id: Some(id),
    spawn: spawn_expansion::<StateRc, SubsRc, FuncRc, O, F, Item, Err, Out, U>,
  };
  let unsub = inner.subscribe(Out::lift(observer)).into_boxed();
  if !unsub.is_closed() {
    subs.rc_deref_mut().insert(id, unsub);
  }
}

impl<StateRc, SubsRc, FuncRc, O, Item, Err, U> Observer<Item, Err>
  for ExpandObserver<StateRc, SubsRc, FuncRc, Item>
where
  StateRc: RcDerefMut<Target = ExpandState<O>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<Item, Err>,
  U: Subscription,
{
  fn next(&mut self, value: Item) { (self.spawn)(&self.state, &self.subs, &self.func, value); }

  fn error(self, err: Err) {
    if let Some(id) = self.id {
      self.subs.rc_deref_mut().remove(id);
    }
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    if let Some(id) = self.id {
      self.subs.rc_deref_mut().remove(id);
    }
    let mut st = self.state.rc_deref_mut();
    match self.id {
      None => st.outer_done = true,
      Some(_) => st.active = st.active.saturating_sub(1),
    }
    st.complete_if_done::<Item, Err>();
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

type StateRc<C> = <C as Context>::RcMut<ExpandState<<C as Context>::Inner>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;
type FuncRc<C, F> = <C as Context>::RcMut<F>;
type Obs<'a, C, F, S> =
  ExpandObserver<StateRc<C>, SubsRc<C>, FuncRc<C, F>, <S as ObservableType>::Item<'a>>;

impl<S, F, C, Out, SourceUnsub> CoreObservable<C> for Expand<S, F>
where
  C: Context,
  S: for<'a> CoreObservable<C::With<Obs<'a, C, F, S>>, Unsub = SourceUnsub>,
  F: for<'a> FnMut(<S as ObservableType>::Item<'a>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<Obs<'a, C, F, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>,
  for<'a> <S as ObservableType>::Item<'a>: Clone,
  SourceUnsub: Subscription,
{
  type Unsub = SourceWithDynamicSubs<SourceUnsub, SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Expand { source, func } = self;
    let state: StateRc<C> = C::RcMut::from(ExpandState {
      observer: Some(context.into_inner()),
      active: 0,
      outer_done: false,
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let func: FuncRc<C, F> = C::RcMut::from(func);
    let observer = ExpandObserver {
      state,
      subs: subs.clone(),
      func,
      id: None,
      spawn: spawn_expansion::<
        StateRc<C>,
        SubsRc<C>,
        FuncRc<C, F>,
        C::Inner,
        F,
        S::Item<'_>,
        S::Err,
        Out,
        C::BoxedSubscription,
      >,
    };
    let source_unsub = source.subscribe(C::lift(observer));
    SourceWithDynamicSubs::new(source_unsub, subs)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_expand_recurses_until_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::of(1)
      .expand(|v| if v < 8 { Local::from_iter(vec![v * 2]) } else { Local::from_iter(vec![]) })
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2, 4, 8]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_expand_fans_out_multiple_children() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::of(1)
      .expand(|v| {
        if v < 4 { Local::from_iter(vec![v * 2, v * 2 + 1]) } else { Local::from_iter(vec![]) }
      })
      .subscribe(move |v| result_c.borrow_mut().push(v));

    // Depth-first because inner observables are synchronous
    assert_eq!(*result.borrow(), vec![1, 2, 4, 5, 3, 6, 7]);
  }

  #[rxrust_macro::test]
  fn test_expand_waits_for_async_inner() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();
    let inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    Local::of(1)
      .expand(move |_| inner_c.clone())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(!*completed.borrow());
    inner.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_expand_unsubscribe_cancels_inner() {
    let inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    let sub = Local::of(1)
      .expand(move |_| inner_c.clone())
      .subscribe(|_| {});
    assert_eq!(inner.inner.subscriber_count(), 1);
    sub.unsubscribe();
    assert_eq!(inner.inner.subscriber_count(), 0);
  }
}
