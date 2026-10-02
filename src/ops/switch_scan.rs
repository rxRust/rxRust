//! SwitchScan operator implementation
//!
//! Like `scan`, but the accumulator returns an observable. Each source item
//! unsubscribes the previous inner observable and subscribes the new one, and
//! every inner emission becomes the new accumulator and is emitted.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDeref, RcDerefMut, Scope},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

#[doc(alias = "switchScan")]
#[derive(Clone)]
pub struct SwitchScan<S, F, Acc> {
  pub source: S,
  pub func: F,
  pub seed: Acc,
}

impl<S, F, Acc> ObservableType for SwitchScan<S, F, Acc>
where
  S: ObservableType,
{
  type Item<'a>
    = Acc
  where
    Self: 'a;
  type Err = S::Err;
}

#[doc(hidden)]
pub struct SwitchScanState<O, Acc, InnerSub> {
  observer: O,
  acc: Acc,
  outer_completed: bool,
  inner_active: bool,
  inner_sub: Option<InnerSub>,
}

impl<O, Acc, InnerSub> Subscription for SwitchScanState<O, Acc, InnerSub>
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

type ScanState<Sc, O, Acc> =
  <Sc as Scope>::RcMut<Option<SwitchScanState<O, Acc, <Sc as Scope>::BoxedSubscription>>>;

#[doc(hidden)]
pub struct SwitchScanOuterObserver<Sc: Scope, O, Acc, F, InnerObs> {
  state: ScanState<Sc, O, Acc>,
  func: F,
  _inner: PhantomData<fn() -> InnerObs>,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct SwitchScanInnerObserver<State>(State);

impl<Sc, O, Acc, Item, Err, F, Out, InnerObs> Observer<Item, Err>
  for SwitchScanOuterObserver<Sc, O, Acc, F, InnerObs>
where
  Sc: Scope,
  O: Observer<Acc, Err>,
  Acc: Clone,
  F: FnMut(Acc, Item) -> Out,
  Out: Context<Inner = InnerObs, Scope = Sc>,
  InnerObs: CoreObservable<
      Out::With<SwitchScanInnerObserver<ScanState<Sc, O, Acc>>>,
      Unsub: IntoBoxedSubscription<Sc::BoxedSubscription>,
    >,
{
  fn next(&mut self, value: Item) {
    let (previous, acc) = {
      let mut guard = self.state.rc_deref_mut();
      let Some(st) = guard.as_mut() else { return };
      if st.observer.is_closed() {
        return;
      }
      st.inner_active = true;
      (st.inner_sub.take(), st.acc.clone())
    };
    // Cancel the previous inner outside the borrow: it may re-enter.
    if let Some(previous) = previous {
      previous.unsubscribe();
    }

    let inner = (self.func)(acc, value).into_inner();
    let unsub = inner
      .subscribe(Out::lift(SwitchScanInnerObserver(self.state.clone())))
      .into_boxed();

    // An inner that completed synchronously has nothing left to cancel.
    if let Some(st) = self.state.rc_deref_mut().as_mut()
      && st.inner_active
    {
      st.inner_sub = Some(unsub);
    }
  }

  fn error(self, err: Err) {
    if let Some(mut st) = self.state.rc_deref_mut().take() {
      let inner = st.inner_sub.take();
      st.observer.error(err);
      if let Some(inner) = inner {
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
      .is_none_or(|st| O::is_closed(&st.observer))
  }
}

impl<Acc, Err, O, State, InnerSub> Observer<Acc, Err> for SwitchScanInnerObserver<State>
where
  O: Observer<Acc, Err>,
  Acc: Clone,
  State: RcDerefMut<Target = Option<SwitchScanState<O, Acc, InnerSub>>> + Clone,
{
  fn next(&mut self, value: Acc) {
    if let Some(st) = self.0.rc_deref_mut().as_mut() {
      st.acc = value.clone();
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
    st.inner_active = false;
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

type InnerCtx<C, Acc> = <C as Context>::With<
  SwitchScanInnerObserver<ScanState<<C as Context>::Scope, <C as Context>::Inner, Acc>>,
>;

impl<S, F, Acc, C, Out, InnerObs> CoreObservable<C> for SwitchScan<S, F, Acc>
where
  C: Context,
  S: CoreObservable<C::With<SwitchScanOuterObserver<C::Scope, C::Inner, Acc, F, InnerObs>>>,
  F: for<'a> FnMut(Acc, S::Item<'a>) -> Out,
  Out: Context<Inner = InnerObs>,
  InnerObs: CoreObservable<InnerCtx<C, Acc>, Err = S::Err> + 'static,
  InnerObs::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  ScanState<C::Scope, C::Inner, Acc>: Subscription,
  Acc: Clone,
{
  type Unsub = TupleSubscription<S::Unsub, ScanState<C::Scope, C::Inner, Acc>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SwitchScan { source, func, seed } = self;
    let state: ScanState<C::Scope, C::Inner, Acc> = <C::Scope as Scope>::RcMut::from(None);

    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() = Some(SwitchScanState {
        observer,
        acc: seed,
        outer_completed: false,
        inner_active: false,
        inner_sub: None,
      });
      SwitchScanOuterObserver { state: state.clone(), func, _inner: PhantomData }
    });

    let source_unsub = source.subscribe(wrapped);
    TupleSubscription::new(source_unsub, state)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_switch_scan_accumulates_through_sync_inners() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let done = Rc::new(RefCell::new(false));
    let (sink, done_c) = (seen.clone(), done.clone());

    Local::from_iter(vec![1, 2, 3])
      .switch_scan(0, |acc: i32, x: i32| Local::of(acc + x))
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(move |v| sink.borrow_mut().push(v));

    assert_eq!(*seen.borrow(), vec![1, 3, 6]);
    assert!(*done.borrow());
  }

  #[rxrust_macro::test]
  fn test_switch_scan_switches_to_the_latest_inner() {
    type Inner = LocalSubject<'static, i32, Infallible>;
    let inners: Rc<RefCell<Vec<Inner>>> = Rc::new(RefCell::new(Vec::new()));
    let seeds: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let done = Rc::new(RefCell::new(false));
    let (inners_c, seeds_c, sink, done_c) =
      (inners.clone(), seeds.clone(), seen.clone(), done.clone());
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .switch_scan(0, move |acc: i32, _x: i32| {
        seeds_c.borrow_mut().push(acc);
        let inner = Local::subject::<i32, Infallible>();
        inners_c.borrow_mut().push(inner.clone());
        inner
      })
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(move |v| sink.borrow_mut().push(v));

    source.next(1);
    let mut first = inners.borrow()[0].clone();
    first.next(10);
    first.next(11);
    assert_eq!(*seen.borrow(), vec![10, 11]);

    // The next source item cancels the first inner and seeds with the latest
    // accumulator.
    source.next(2);
    assert_eq!(first.inner.subscriber_count(), 0);
    assert_eq!(*seeds.borrow(), vec![0, 11]);
    first.next(12); // ignored: unsubscribed
    let mut second = inners.borrow()[1].clone();
    second.next(20);
    assert_eq!(*seen.borrow(), vec![10, 11, 20]);

    // Completion waits for the active inner.
    source.complete();
    assert!(!*done.borrow());
    second.complete();
    assert!(*done.borrow());
  }

  #[rxrust_macro::test]
  fn test_switch_scan_completes_when_source_completes_after_inner() {
    let done = Rc::new(RefCell::new(false));
    let done_c = done.clone();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .switch_scan(0, |acc: i32, x: i32| Local::of(acc + x))
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(|_| {});

    source.next(1);
    assert!(!*done.borrow());
    source.complete();
    assert!(*done.borrow());
  }

  #[rxrust_macro::test]
  fn test_switch_scan_propagates_inner_error() {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let errors_c = errors.clone();
    let mut source = Local::subject::<i32, &'static str>();
    let inner = Local::subject::<i32, &'static str>();
    let inner_c = inner.clone();

    source
      .clone()
      .switch_scan(0, move |_acc: i32, _x: i32| inner_c.clone())
      .on_error(move |e| errors_c.borrow_mut().push(e))
      .subscribe(|_| {});

    source.next(1);
    inner.clone().error("boom");
    assert_eq!(*errors.borrow(), vec!["boom"]);
    // Later source items are ignored by the closed operator.
    source.next(2);
    assert_eq!(*errors.borrow(), vec!["boom"]);
  }
}
