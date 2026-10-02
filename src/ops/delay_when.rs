//! DelayWhen operator implementation
//!
//! Delays each item until an observable chosen for that item emits or
//! completes.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// DelayWhen operator: Delay every item by its own duration observable
///
/// For each item, `selector(&item)` returns an observable; the item is
/// emitted when that observable first emits or completes, as in RxJS 7.
/// Items may be reordered if their delays differ. Completes once the source
/// has completed and every pending delay has resolved.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter(vec![1, 2])
///   .delay_when(|_| Local::of(()))
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "delayWhen")]
#[derive(Clone)]
pub struct DelayWhen<S, F> {
  pub source: S,
  pub selector: F,
}

impl<S, F> ObservableType for DelayWhen<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// State shared by the source and delay observers
pub struct DelayWhenState<O> {
  observer: Option<O>,
  pending: usize,
  outer_done: bool,
}

impl<O> DelayWhenState<O> {
  fn complete_if_done<Item, Err>(&mut self)
  where
    O: Observer<Item, Err>,
  {
    if self.outer_done
      && self.pending == 0
      && let Some(observer) = self.observer.take()
    {
      observer.complete();
    }
  }
}

/// Observer for one item's delay observable
pub struct DelayWhenInnerObserver<StateRc, SubsRc, Item> {
  state: StateRc,
  subs: SubsRc,
  id: usize,
  value: Option<Item>,
}

impl<StateRc, SubsRc, Item, O, U> DelayWhenInnerObserver<StateRc, SubsRc, Item>
where
  StateRc: RcDerefMut<Target = DelayWhenState<O>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
{
  fn fire<Err>(&mut self)
  where
    O: Observer<Item, Err>,
  {
    let Some(value) = self.value.take() else { return };
    let mut st = self.state.rc_deref_mut();
    if let Some(observer) = st.observer.as_mut() {
      observer.next(value);
    }
    st.pending -= 1;
    st.complete_if_done::<Item, Err>();
    drop(st);
    // Drop our own handle rather than unsubscribing mid-dispatch.
    self.subs.rc_deref_mut().remove(self.id);
  }
}

impl<StateRc, SubsRc, Item, O, Err, DelayItem, U> Observer<DelayItem, Err>
  for DelayWhenInnerObserver<StateRc, SubsRc, Item>
where
  StateRc: RcDerefMut<Target = DelayWhenState<O>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<Item, Err>,
  U: Subscription,
{
  fn next(&mut self, _value: DelayItem) { self.fire::<Err>(); }

  fn error(self, err: Err) {
    self.subs.rc_deref_mut().remove(self.id);
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(mut self) { self.fire::<Err>(); }

  fn is_closed(&self) -> bool {
    self.value.is_none()
      || self
        .state
        .rc_deref()
        .observer
        .as_ref()
        .is_none_or(|o| o.is_closed())
  }
}

/// Observer for the source
pub struct DelayWhenSourceObserver<StateRc, SubsRc, F> {
  state: StateRc,
  subs: SubsRc,
  selector: F,
}

impl<StateRc, SubsRc, F, O, Item, Err, Out, U> Observer<Item, Err>
  for DelayWhenSourceObserver<StateRc, SubsRc, F>
where
  StateRc: RcDerefMut<Target = DelayWhenState<O>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<DelayWhenInnerObserver<StateRc, SubsRc, Item>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
{
  fn next(&mut self, value: Item) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let delay = (self.selector)(&value).into_inner();
    let id = self.subs.rc_deref_mut().reserve_id();
    self.state.rc_deref_mut().pending += 1;
    let observer = DelayWhenInnerObserver {
      state: self.state.clone(),
      subs: self.subs.clone(),
      id,
      value: Some(value),
    };
    let unsub = delay.subscribe(Out::lift(observer)).into_boxed();
    // A delay that resolved synchronously has nothing left to cancel.
    if !unsub.is_closed() {
      self.subs.rc_deref_mut().insert(id, unsub);
    }
  }

  fn error(self, err: Err) {
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    let mut st = self.state.rc_deref_mut();
    st.outer_done = true;
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

type StateRc<C> = <C as Context>::RcMut<DelayWhenState<<C as Context>::Inner>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;
type DwInner<'a, C, S> =
  DelayWhenInnerObserver<StateRc<C>, SubsRc<C>, <S as ObservableType>::Item<'a>>;

impl<S, F, C, Out> CoreObservable<C> for DelayWhen<S, F>
where
  C: Context,
  S: CoreObservable<C::With<DelayWhenSourceObserver<StateRc<C>, SubsRc<C>, F>>>,
  F: for<'a> FnMut(&<S as ObservableType>::Item<'a>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<DwInner<'a, C, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>,
{
  type Unsub = SourceWithDynamicSubs<S::Unsub, SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DelayWhen { source, selector } = self;
    let state: StateRc<C> = C::RcMut::from(DelayWhenState {
      observer: Some(context.into_inner()),
      pending: 0,
      outer_done: false,
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let observer = DelayWhenSourceObserver { state, subs: subs.clone(), selector };
    let source_unsub = source.subscribe(C::lift(observer));
    SourceWithDynamicSubs::new(source_unsub, subs)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  #[rxrust_macro::test]
  fn test_delay_when_reorders_by_delay() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let _sub = TestCtx::from_iter(vec![30u64, 10, 20])
      .delay_when(|ms| TestCtx::timer(Duration::from_millis(*ms)))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert!(result.borrow().is_empty());
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(*result.borrow(), vec![10]);
    assert!(!*completed.borrow());
    TestScheduler::advance_by(Duration::from_millis(20));
    assert_eq!(*result.borrow(), vec![10, 20, 30]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_delay_when_completion_of_duration_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(vec![1, 2])
      .delay_when(|_| Local::from_iter(Vec::<()>::new()))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_delay_when_error_from_duration() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::from_iter(vec![1])
      .map_err(|_: Infallible| String::new())
      .delay_when(|_| Local::throw_err("late".to_string()).map(|_| ()))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(error.borrow().as_deref(), Some("late"));
  }

  #[rxrust_macro::test]
  fn test_delay_when_unsubscribe_cancels_pending() {
    let source = Local::subject::<i32, Infallible>();
    let delay = Local::subject::<(), Infallible>();
    let delay_c = delay.clone();
    let mut source_emitter = source.clone();

    let sub = source
      .clone()
      .delay_when(move |_| delay_c.clone())
      .subscribe(|_| {});
    source_emitter.next(1);
    assert_eq!(delay.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert_eq!(delay.inner.subscriber_count(), 0);
    assert_eq!(source.inner.subscriber_count(), 0);
  }
}
