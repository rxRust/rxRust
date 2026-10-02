//! WindowToggle operator implementation
//!
//! Overlapping windows: every item from `openings` opens a window, closed
//! when the observable chosen for that opening emits or completes. Each
//! window is a `Subject` wrapped in the context.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
    TupleSubscription,
  },
};

#[doc(alias = "windowToggle")]
pub struct WindowToggle<S, Op, F, CtxMarker> {
  pub source: S,
  pub openings: Op,
  pub closing_selector: F,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<S: Clone, Op: Clone, F: Clone, CtxMarker> Clone for WindowToggle<S, Op, F, CtxMarker> {
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      openings: self.openings.clone(),
      closing_selector: self.closing_selector.clone(),
      _marker: PhantomData,
    }
  }
}

impl<S, Op, F, CtxMarker> WindowToggle<S, Op, F, CtxMarker> {
  pub fn new(source: S, openings: Op, closing_selector: F) -> Self {
    Self { source, openings, closing_selector, _marker: PhantomData }
  }
}

impl<S, Op, F, CtxMarker> ObservableType for WindowToggle<S, Op, F, CtxMarker>
where
  S: ObservableType,
  CtxMarker: Context,
{
  type Item<'m>
    = CtxMarker::With<CtxMarker::Inner>
  where
    Self: 'm;
  type Err = S::Err;
}

pub struct WindowToggleState<O, Subj> {
  observer: Option<O>,
  windows: Vec<(usize, Subj)>,
}

impl<O, Subj> WindowToggleState<O, Subj> {
  fn close(&mut self, id: usize) -> Option<Subj> {
    let pos = self
      .windows
      .iter()
      .position(|(wid, _)| *wid == id)?;
    Some(self.windows.remove(pos).1)
  }

  fn take_all(&mut self) -> (Vec<Subj>, Option<O>) {
    let windows = std::mem::take(&mut self.windows)
      .into_iter()
      .map(|(_, w)| w)
      .collect();
    (windows, self.observer.take())
  }

  fn open_windows(&self) -> Vec<Subj>
  where
    Subj: Clone,
  {
    self
      .windows
      .iter()
      .map(|(_, w)| w.clone())
      .collect()
  }

  fn is_open(&self, id: usize) -> bool { self.windows.iter().any(|(wid, _)| *wid == id) }
}

/// Observer for one closing observable
pub struct WindowToggleClosingObserver<StateRc, SubsRc, CtxMarker, Item> {
  state: StateRc,
  subs: SubsRc,
  id: usize,
  _marker: PhantomData<fn() -> (CtxMarker, Item)>,
}

impl<StateRc, SubsRc, CtxMarker, Item, O, Err, NotifyItem, U> Observer<NotifyItem, Err>
  for WindowToggleClosingObserver<StateRc, SubsRc, CtxMarker, Item>
where
  CtxMarker: Context<Inner: Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowToggleState<O, CtxMarker::Inner>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  U: Subscription,
  Err: Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    let window = self.state.rc_deref_mut().close(self.id);
    if let Some(window) = window {
      window.complete();
    }
    // Drop our own handle rather than unsubscribing mid-dispatch.
    self.subs.rc_deref_mut().remove(self.id);
  }

  fn error(self, err: Err) {
    self.subs.rc_deref_mut().remove(self.id);
    let (windows, observer) = self.state.rc_deref_mut().take_all();
    for window in windows {
      window.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(mut self) {
    // Closing completion closes the window, as in RxJS.
    self.next(());
  }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.observer.as_ref().is_none_or(|o| o.is_closed()) || !st.is_open(self.id)
  }
}

/// Observer for the openings observable
pub struct WindowToggleOpeningsObserver<StateRc, SubsRc, F, CtxMarker, Item> {
  state: StateRc,
  subs: SubsRc,
  closing_selector: F,
  _marker: PhantomData<fn() -> (CtxMarker, Item)>,
}

impl<StateRc, SubsRc, F, CtxMarker, Item, O, Err, OpenItem, Out, U> Observer<OpenItem, Err>
  for WindowToggleOpeningsObserver<StateRc, SubsRc, F, CtxMarker, Item>
where
  CtxMarker: Context<Inner: Default + Clone + Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowToggleState<O, CtxMarker::Inner>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  F: FnMut(OpenItem) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<WindowToggleClosingObserver<StateRc, SubsRc, CtxMarker, Item>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
  Err: Clone,
{
  fn next(&mut self, opening: OpenItem) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let id = self.subs.rc_deref_mut().reserve_id();
    let window = CtxMarker::Inner::default();
    {
      let mut st = self.state.rc_deref_mut();
      st.windows.push((id, window.clone()));
      if let Some(observer) = st.observer.as_mut() {
        observer.next(CtxMarker::lift(window));
      }
    }
    let closing = (self.closing_selector)(opening).into_inner();
    let observer = WindowToggleClosingObserver {
      state: self.state.clone(),
      subs: self.subs.clone(),
      id,
      _marker: PhantomData,
    };
    let unsub = closing
      .subscribe(Out::lift(observer))
      .into_boxed();
    if self.state.rc_deref().is_open(id) {
      self.subs.rc_deref_mut().insert(id, unsub);
    }
  }

  fn error(self, err: Err) {
    let (windows, observer) = self.state.rc_deref_mut().take_all();
    for window in windows {
      window.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    // Openings ending does not end the windows already open.
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
pub struct WindowToggleSourceObserver<StateRc, SubsRc, OpUnsub, CtxMarker> {
  state: StateRc,
  subs: SubsRc,
  openings_unsub: OpUnsub,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<StateRc, SubsRc, OpUnsub, CtxMarker, O, Item, Err, U> Observer<Item, Err>
  for WindowToggleSourceObserver<StateRc, SubsRc, OpUnsub, CtxMarker>
where
  CtxMarker: Context<Inner: Clone + Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowToggleState<O, CtxMarker::Inner>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  OpUnsub: Subscription,
  U: Subscription,
  Item: Clone,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    // Snapshot the windows so their subscribers may touch the operator.
    let windows = self.state.rc_deref().open_windows();
    for mut window in windows {
      window.next(value.clone());
    }
  }

  fn error(self, err: Err) {
    self.openings_unsub.unsubscribe();
    let (windows, observer) = self.state.rc_deref_mut().take_all();
    for window in windows {
      window.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    self.openings_unsub.unsubscribe();
    let (windows, observer) = self.state.rc_deref_mut().take_all();
    for window in windows {
      window.complete();
    }
    if let Some(observer) = observer {
      observer.complete();
    }
    self.subs.rc_deref_mut().unsubscribe_all();
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

type StateRc<C, CtxMarker> =
  <C as Context>::RcMut<WindowToggleState<<C as Context>::Inner, <CtxMarker as Context>::Inner>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;
type OpProxy<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type WtSource<C, CtxMarker> =
  WindowToggleSourceObserver<StateRc<C, CtxMarker>, SubsRc<C>, OpProxy<C>, CtxMarker>;
type WtOpenings<'a, C, CtxMarker, F, S> = WindowToggleOpeningsObserver<
  StateRc<C, CtxMarker>,
  SubsRc<C>,
  F,
  CtxMarker,
  <S as ObservableType>::Item<'a>,
>;
type WtClosing<'a, C, CtxMarker, S> = WindowToggleClosingObserver<
  StateRc<C, CtxMarker>,
  SubsRc<C>,
  CtxMarker,
  <S as ObservableType>::Item<'a>,
>;

impl<S, Op, F, CtxMarker, C, Out, SourceUnsub> CoreObservable<C>
  for WindowToggle<S, Op, F, CtxMarker>
where
  C: Context,
  CtxMarker:
    Context<Inner: Default + Clone + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>>,
  S: CoreObservable<C::With<WtSource<C, CtxMarker>>, Unsub = SourceUnsub>,
  Op: ObservableType<Err = S::Err>
    + for<'a> CoreObservable<
      C::With<WtOpenings<'a, C, CtxMarker, F, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  F: for<'b> FnMut(<Op as ObservableType>::Item<'b>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<WtClosing<'a, C, CtxMarker, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: Observer<CtxMarker::With<CtxMarker::Inner>, S::Err>,
  for<'a> <S as ObservableType>::Item<'a>: Clone,
  S::Err: Clone,
  SourceUnsub: Subscription,
  OpProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<SourceWithDynamicSubs<SourceUnsub, SubsRc<C>>, OpProxy<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let WindowToggle { source, openings, closing_selector, .. } = self;
    let state: StateRc<C, CtxMarker> = C::RcMut::from(WindowToggleState {
      observer: Some(context.into_inner()),
      windows: Vec::new(),
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let op_proxy: OpProxy<C> = C::RcMut::from(None);

    let openings_observer = WindowToggleOpeningsObserver {
      state: state.clone(),
      subs: subs.clone(),
      closing_selector,
      _marker: PhantomData,
    };
    let op_unsub = openings
      .subscribe(C::lift(openings_observer))
      .into_boxed();
    *op_proxy.rc_deref_mut() = Some(op_unsub);

    let source_observer = WindowToggleSourceObserver {
      state,
      subs: subs.clone(),
      openings_unsub: op_proxy.clone(),
      _marker: PhantomData,
    };
    let source_unsub = source.subscribe(C::lift(source_observer));

    TupleSubscription::new(SourceWithDynamicSubs::new(source_unsub, subs), op_proxy)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  type Buckets = Rc<RefCell<Vec<Rc<RefCell<Vec<i32>>>>>>;

  fn snapshot(buckets: &Buckets) -> Vec<Vec<i32>> {
    buckets
      .borrow()
      .iter()
      .map(|b| b.borrow().clone())
      .collect()
  }

  #[rxrust_macro::test]
  fn test_window_toggle_windows_overlap() {
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let sink = buckets.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<&'static str, Infallible>();
    let closings: Rc<RefCell<Vec<LocalSubject<'static, (), Infallible>>>> =
      Rc::new(RefCell::new(Vec::new()));
    let closings_c = closings.clone();

    source
      .clone()
      .window_toggle(openings.clone(), move |_name| {
        let closing = Local::subject::<(), Infallible>();
        closings_c.borrow_mut().push(closing.clone());
        closing
      })
      .subscribe(move |w: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        sink.borrow_mut().push(bucket.clone());
        w.subscribe(move |v| bucket.borrow_mut().push(v));
      });

    source.next(0); // no window open yet
    openings.next("a");
    source.next(1);
    openings.next("b");
    source.next(2);
    closings.borrow()[0].clone().next(()); // close a
    source.next(3);
    assert_eq!(snapshot(&buckets), vec![vec![1, 2], vec![2, 3]]);

    source.complete();
    assert_eq!(snapshot(&buckets), vec![vec![1, 2], vec![2, 3]]);
    assert_eq!(openings.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_window_toggle_completion_completes_open_windows() {
    let completed = Rc::new(RefCell::new(0));
    let outer_done = Rc::new(RefCell::new(false));
    let (completed_c, outer_c) = (completed.clone(), outer_done.clone());
    let source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<(), Infallible>();
    let never = Local::subject::<(), Infallible>();

    source
      .clone()
      .window_toggle(openings.clone(), move |_| never.clone())
      .on_complete(move || *outer_c.borrow_mut() = true)
      .subscribe(move |w: Local<_>| {
        let c = completed_c.clone();
        w.on_complete(move || *c.borrow_mut() += 1)
          .subscribe(|_| {});
      });

    openings.next(());
    openings.next(());
    source.complete();

    assert_eq!(*completed.borrow(), 2);
    assert!(*outer_done.borrow());
  }

  #[rxrust_macro::test]
  fn test_window_toggle_closing_completion_closes_window() {
    let completed = Rc::new(RefCell::new(0));
    let completed_c = completed.clone();
    let source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<(), Infallible>();
    let closing = Local::subject::<(), Infallible>();
    let closing_c = closing.clone();

    source
      .clone()
      .window_toggle(openings.clone(), move |_| closing_c.clone())
      .subscribe(move |w: Local<_>| {
        let c = completed_c.clone();
        w.on_complete(move || *c.borrow_mut() += 1)
          .subscribe(|_| {});
      });

    openings.next(());
    assert_eq!(*completed.borrow(), 0);
    closing.clone().complete();
    assert_eq!(*completed.borrow(), 1, "closing completion closes the window");
  }

  #[rxrust_macro::test]
  fn test_window_toggle_error_reaches_windows_and_outer() {
    let window_errors = Rc::new(RefCell::new(Vec::new()));
    let outer_errors = Rc::new(RefCell::new(Vec::new()));
    let (we, oe) = (window_errors.clone(), outer_errors.clone());
    let source = Local::subject::<i32, &'static str>();
    let mut openings = Local::subject::<(), &'static str>();
    let never = Local::subject::<(), &'static str>();

    source
      .clone()
      .window_toggle(openings.clone(), move |_| never.clone())
      .on_error(move |e| oe.borrow_mut().push(e))
      .subscribe(move |w: Local<_>| {
        let we = we.clone();
        w.on_error(move |e| we.borrow_mut().push(e))
          .subscribe(|_| {});
      });

    openings.next(());
    source.error("boom");
    assert_eq!(*window_errors.borrow(), vec!["boom"]);
    assert_eq!(*outer_errors.borrow(), vec!["boom"]);
    assert_eq!(openings.inner.subscriber_count(), 0);
  }
}
