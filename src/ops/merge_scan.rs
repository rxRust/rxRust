//! MergeScan operator implementation
//!
//! Like `scan`, but the accumulator function returns an observable whose
//! emissions become the new accumulator.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// MergeScan operator: Accumulate through observables
///
/// For each item, `f(acc, item)` returns an observable of accumulators; each
/// of its emissions becomes the current accumulator and is emitted. Inner
/// observables run concurrently. Completes when the source and every inner
/// observable have completed.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter(vec![1, 2, 3])
///   .merge_scan(0, |acc, v| Local::of(acc + v))
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 3, 6]);
/// ```
#[doc(alias = "mergeScan")]
#[derive(Clone)]
pub struct MergeScan<S, F, Acc> {
  pub source: S,
  pub func: F,
  pub seed: Acc,
}

impl<S, F, Acc> ObservableType for MergeScan<S, F, Acc>
where
  S: ObservableType,
{
  type Item<'a>
    = Acc
  where
    Self: 'a;
  type Err = S::Err;
}

/// State shared by the source and inner observers
pub struct MergeScanState<O, Acc> {
  observer: Option<O>,
  acc: Acc,
  active: usize,
  outer_done: bool,
}

impl<O, Acc> MergeScanState<O, Acc> {
  fn complete_if_done<Err>(&mut self)
  where
    O: Observer<Acc, Err>,
  {
    if self.outer_done
      && self.active == 0
      && let Some(observer) = self.observer.take()
    {
      observer.complete();
    }
  }
}

/// Observer for one inner observable
pub struct MergeScanInnerObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  id: usize,
}

impl<StateRc, SubsRc, O, Acc, Err, U> Observer<Acc, Err> for MergeScanInnerObserver<StateRc, SubsRc>
where
  StateRc: RcDerefMut<Target = MergeScanState<O, Acc>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<Acc, Err>,
  Acc: Clone,
  U: Subscription,
{
  fn next(&mut self, value: Acc) {
    let mut st = self.state.rc_deref_mut();
    if st.observer.is_none() {
      return;
    }
    st.acc = value.clone();
    if let Some(observer) = st.observer.as_mut() {
      observer.next(value);
    }
  }

  fn error(self, err: Err) {
    self.subs.rc_deref_mut().remove(self.id);
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    self.subs.rc_deref_mut().remove(self.id);
    let mut st = self.state.rc_deref_mut();
    st.active = st.active.saturating_sub(1);
    st.complete_if_done::<Err>();
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

/// Observer for the source
pub struct MergeScanSourceObserver<StateRc, SubsRc, F> {
  state: StateRc,
  subs: SubsRc,
  func: F,
}

impl<StateRc, SubsRc, F, O, Acc, Item, Err, Out, U> Observer<Item, Err>
  for MergeScanSourceObserver<StateRc, SubsRc, F>
where
  StateRc: RcDerefMut<Target = MergeScanState<O, Acc>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  O: Observer<Acc, Err>,
  Acc: Clone,
  F: FnMut(Acc, Item) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<MergeScanInnerObserver<StateRc, SubsRc>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
{
  fn next(&mut self, value: Item) {
    let acc = {
      let st = self.state.rc_deref();
      if st.observer.is_none() {
        return;
      }
      st.acc.clone()
    };
    let inner = (self.func)(acc, value).into_inner();
    let id = self.subs.rc_deref_mut().reserve_id();
    self.state.rc_deref_mut().active += 1;
    let observer =
      MergeScanInnerObserver { state: self.state.clone(), subs: self.subs.clone(), id };
    let unsub = inner.subscribe(Out::lift(observer)).into_boxed();
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
    st.complete_if_done::<Err>();
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

type StateRc<C, Acc> = <C as Context>::RcMut<MergeScanState<<C as Context>::Inner, Acc>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

impl<S, F, Acc, C, Out> CoreObservable<C> for MergeScan<S, F, Acc>
where
  C: Context,
  S: CoreObservable<C::With<MergeScanSourceObserver<StateRc<C, Acc>, SubsRc<C>, F>>>,
  F: for<'a> FnMut(Acc, <S as ObservableType>::Item<'a>) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<MergeScanInnerObserver<StateRc<C, Acc>, SubsRc<C>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: Observer<Acc, S::Err>,
  Acc: Clone,
{
  type Unsub = SourceWithDynamicSubs<S::Unsub, SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let MergeScan { source, func, seed } = self;
    let state: StateRc<C, Acc> = C::RcMut::from(MergeScanState {
      observer: Some(context.into_inner()),
      acc: seed,
      active: 0,
      outer_done: false,
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let observer = MergeScanSourceObserver { state, subs: subs.clone(), func };
    let source_unsub = source.subscribe(C::lift(observer));
    SourceWithDynamicSubs::new(source_unsub, subs)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_merge_scan_sync_accumulates() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter(vec![1, 2, 3])
      .merge_scan(0, |acc, v| Local::of(acc + v))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 3, 6]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_merge_scan_multiple_inner_emissions() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(vec![1, 2])
      .merge_scan(0, |acc, v| Local::from_iter(vec![acc + v, acc + v * 10]))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    // 0+1=1, 0+10=10; then acc=10: 10+2=12, 10+20=30
    assert_eq!(*result.borrow(), vec![1, 10, 12, 30]);
  }

  #[rxrust_macro::test]
  fn test_merge_scan_waits_for_inner_completion() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let inner = Local::subject::<i32, Infallible>();
    let inner_c = inner.clone();

    source
      .clone()
      .merge_scan(0, move |_, _| inner_c.clone())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    source.next(1);
    source.clone().complete();
    assert!(!*completed.borrow());
    inner.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_merge_scan_inner_error() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::from_iter(vec![1])
      .map_err(|_: Infallible| String::new())
      .merge_scan(0, |_, _| Local::throw_err("boom".to_string()).map(|_| 0))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(error.borrow().as_deref(), Some("boom"));
  }
}
