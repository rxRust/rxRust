//! WindowWhen operator implementation
//!
//! Consecutive windows, each closed by an observable obtained from a selector
//! when the window opens. The first window opens at subscribe; a closing
//! emission completes the window and opens the next.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

#[doc(alias = "windowWhen")]
pub struct WindowWhen<S, F, CtxMarker> {
  pub source: S,
  pub closing_selector: F,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<S: Clone, F: Clone, CtxMarker> Clone for WindowWhen<S, F, CtxMarker> {
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      closing_selector: self.closing_selector.clone(),
      _marker: PhantomData,
    }
  }
}

impl<S, F, CtxMarker> WindowWhen<S, F, CtxMarker> {
  pub fn new(source: S, closing_selector: F) -> Self {
    Self { source, closing_selector, _marker: PhantomData }
  }
}

impl<S, F, CtxMarker> ObservableType for WindowWhen<S, F, CtxMarker>
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

pub struct WindowWhenState<O, Subj> {
  observer: Option<O>,
  current: Option<Subj>,
  generation: usize,
}

pub struct WindowWhenClosingObserver<StateRc, SelRc, SlotRc, CtxMarker, Item> {
  state: StateRc,
  selector: SelRc,
  slot: SlotRc,
  generation: usize,
  reopen: fn(StateRc, SelRc, SlotRc),
  _marker: PhantomData<fn() -> (CtxMarker, Item)>,
}

/// Complete the current window (if any), open the next one and subscribe
/// its closing observable.
fn open_window<Out, StateRc, SelRc, SlotRc, CtxMarker, Item, Err, O, F, BoxedSub>(
  state: StateRc, selector: SelRc, slot: SlotRc,
) where
  CtxMarker: Context<Inner: Default + Clone + Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowWhenState<O, CtxMarker::Inner>> + Clone,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  SelRc: RcDerefMut<Target = F> + Clone,
  SlotRc: RcDerefMut<Target = Option<BoxedSub>> + Clone,
  F: FnMut() -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<WindowWhenClosingObserver<StateRc, SelRc, SlotRc, CtxMarker, Item>>,
      Unsub: IntoBoxedSubscription<BoxedSub>,
    >,
  >,
{
  let previous = state.rc_deref_mut().current.take();
  if let Some(previous) = previous {
    previous.complete();
  }
  let generation = {
    let mut st = state.rc_deref_mut();
    let Some(observer) = st.observer.as_mut() else { return };
    if observer.is_closed() {
      return;
    }
    let window = CtxMarker::Inner::default();
    observer.next(CtxMarker::lift(window.clone()));
    st.current = Some(window);
    st.generation += 1;
    st.generation
  };
  let closing = (selector.rc_deref_mut())().into_inner();
  let observer = WindowWhenClosingObserver {
    state: state.clone(),
    selector: selector.clone(),
    slot: slot.clone(),
    generation,
    reopen: open_window::<Out, StateRc, SelRc, SlotRc, CtxMarker, Item, Err, O, F, BoxedSub>,
    _marker: PhantomData,
  };
  let unsub = closing.subscribe(Out::lift(observer));
  // A stale handle is simply replaced; the old closing sees `is_closed`.
  *slot.rc_deref_mut() = Some(unsub.into_boxed());
}

impl<StateRc, SelRc, SlotRc, CtxMarker, Item, O, Err, NotifyItem> Observer<NotifyItem, Err>
  for WindowWhenClosingObserver<StateRc, SelRc, SlotRc, CtxMarker, Item>
where
  CtxMarker: Context<Inner: Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowWhenState<O, CtxMarker::Inner>> + Clone,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  SelRc: Clone,
  SlotRc: Clone,
  Err: Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    {
      let st = self.state.rc_deref();
      if st.generation != self.generation || st.observer.is_none() {
        return;
      }
    }
    (self.reopen)(self.state.clone(), self.selector.clone(), self.slot.clone());
  }

  fn error(self, err: Err) {
    let (window, observer) = {
      let mut st = self.state.rc_deref_mut();
      (st.current.take(), st.observer.take())
    };
    if let Some(window) = window {
      window.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    // Closing completion without a value keeps the window open, as in RxJS.
  }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.generation != self.generation || st.observer.as_ref().is_none_or(|o| o.is_closed())
  }
}

pub struct WindowWhenSourceObserver<StateRc, SlotRc, CtxMarker> {
  state: StateRc,
  slot: SlotRc,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<StateRc, SlotRc, CtxMarker, O, Item, Err> Observer<Item, Err>
  for WindowWhenSourceObserver<StateRc, SlotRc, CtxMarker>
where
  CtxMarker: Context<Inner: Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = WindowWhenState<O, CtxMarker::Inner>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  SlotRc: Subscription,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    if let Some(window) = self.state.rc_deref_mut().current.as_mut() {
      window.next(value);
    }
  }

  fn error(self, err: Err) {
    self.slot.unsubscribe();
    let (window, observer) = {
      let mut st = self.state.rc_deref_mut();
      st.generation += 1;
      (st.current.take(), st.observer.take())
    };
    if let Some(window) = window {
      window.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.slot.unsubscribe();
    let (window, observer) = {
      let mut st = self.state.rc_deref_mut();
      st.generation += 1;
      (st.current.take(), st.observer.take())
    };
    if let Some(window) = window {
      window.complete();
    }
    if let Some(observer) = observer {
      observer.complete();
    }
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
  <C as Context>::RcMut<WindowWhenState<<C as Context>::Inner, <CtxMarker as Context>::Inner>>;
type SelRc<C, F> = <C as Context>::RcMut<F>;
type SlotRc<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type WwSource<C, CtxMarker> = WindowWhenSourceObserver<StateRc<C, CtxMarker>, SlotRc<C>, CtxMarker>;
type WwClosing<'a, C, CtxMarker, F, S> = WindowWhenClosingObserver<
  StateRc<C, CtxMarker>,
  SelRc<C, F>,
  SlotRc<C>,
  CtxMarker,
  <S as ObservableType>::Item<'a>,
>;

impl<S, F, CtxMarker, C, Out, SourceUnsub> CoreObservable<C> for WindowWhen<S, F, CtxMarker>
where
  C: Context,
  CtxMarker:
    Context<Inner: Default + Clone + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>>,
  S: CoreObservable<C::With<WwSource<C, CtxMarker>>, Unsub = SourceUnsub>,
  F: FnMut() -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<WwClosing<'a, C, CtxMarker, F, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: Observer<CtxMarker::With<CtxMarker::Inner>, S::Err>,
  S::Err: Clone,
  SourceUnsub: Subscription,
  SlotRc<C>: Subscription,
{
  type Unsub = TupleSubscription<SourceUnsub, SlotRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let WindowWhen { source, closing_selector, .. } = self;
    let state: StateRc<C, CtxMarker> = C::RcMut::from(WindowWhenState {
      observer: Some(context.into_inner()),
      current: None,
      generation: 0,
    });
    let selector: SelRc<C, F> = C::RcMut::from(closing_selector);
    let slot: SlotRc<C> = C::RcMut::from(None);

    open_window::<Out, _, _, _, CtxMarker, S::Item<'_>, S::Err, C::Inner, F, C::BoxedSubscription>(
      state.clone(),
      selector,
      slot.clone(),
    );

    let source_observer =
      WindowWhenSourceObserver { state, slot: slot.clone(), _marker: PhantomData };
    let source_unsub = source.subscribe(C::lift(source_observer));
    TupleSubscription::new(source_unsub, slot)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  type Buckets = Rc<RefCell<Vec<Rc<RefCell<Vec<i32>>>>>>;
  type Closings = Rc<RefCell<Vec<LocalSubject<'static, (), Infallible>>>>;

  fn snapshot(buckets: &Buckets) -> Vec<Vec<i32>> {
    buckets
      .borrow()
      .iter()
      .map(|b| b.borrow().clone())
      .collect()
  }

  fn wire(source: &LocalSubject<'static, i32, Infallible>) -> (Buckets, Closings) {
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let closings: Closings = Rc::new(RefCell::new(Vec::new()));
    let (sink, closings_c) = (buckets.clone(), closings.clone());
    source
      .clone()
      .window_when(move || {
        let closing = Local::subject::<(), Infallible>();
        closings_c.borrow_mut().push(closing.clone());
        closing
      })
      .subscribe(move |w: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        sink.borrow_mut().push(bucket.clone());
        w.subscribe(move |v| bucket.borrow_mut().push(v));
      });
    (buckets, closings)
  }

  #[rxrust_macro::test]
  fn test_window_when_rotates_on_closing_emission() {
    let mut source = Local::subject::<i32, Infallible>();
    let (buckets, closings) = wire(&source);

    // The first window and its closing exist at subscribe.
    assert_eq!(snapshot(&buckets), vec![Vec::<i32>::new()]);
    assert_eq!(closings.borrow().len(), 1);

    source.next(1);
    source.next(2);
    let mut first = closings.borrow()[0].clone();
    first.next(()); // reopen pushes the next closing: hold no borrow here
    source.next(3);
    let mut second = closings.borrow()[1].clone();
    second.next(());
    source.complete();

    assert_eq!(snapshot(&buckets), vec![vec![1, 2], vec![3], vec![]]);
    assert_eq!(closings.borrow().len(), 3);
    assert_eq!(closings.borrow()[2].inner.subscriber_count(), 0, "completion releases the closing");
  }

  #[rxrust_macro::test]
  fn test_window_when_closing_completion_keeps_window_open() {
    let mut source = Local::subject::<i32, Infallible>();
    let (buckets, closings) = wire(&source);

    closings.borrow()[0].clone().complete();
    source.next(1);
    source.complete();

    assert_eq!(snapshot(&buckets), vec![vec![1]]);
  }

  #[rxrust_macro::test]
  fn test_window_when_completes_window_and_outer() {
    let completed = Rc::new(RefCell::new(0));
    let outer_done = Rc::new(RefCell::new(false));
    let (completed_c, outer_c) = (completed.clone(), outer_done.clone());
    let mut source = Local::subject::<i32, Infallible>();
    let closing = Local::subject::<(), Infallible>();
    let closing_c = closing.clone();

    source
      .clone()
      .window_when(move || closing_c.clone())
      .on_complete(move || *outer_c.borrow_mut() = true)
      .subscribe(move |w: Local<_>| {
        let c = completed_c.clone();
        w.on_complete(move || *c.borrow_mut() += 1)
          .subscribe(|_| {});
      });

    source.next(1);
    source.complete();

    assert_eq!(*completed.borrow(), 1);
    assert!(*outer_done.borrow());
    assert_eq!(closing.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_window_when_error_reaches_window_and_outer() {
    let window_errors = Rc::new(RefCell::new(Vec::new()));
    let outer_errors = Rc::new(RefCell::new(Vec::new()));
    let (we, oe) = (window_errors.clone(), outer_errors.clone());
    let source = Local::subject::<i32, &'static str>();
    let closing = Local::subject::<(), &'static str>();
    let closing_c = closing.clone();

    source
      .clone()
      .window_when(move || closing_c.clone())
      .on_error(move |e| oe.borrow_mut().push(e))
      .subscribe(move |w: Local<_>| {
        let we = we.clone();
        w.on_error(move |e| we.borrow_mut().push(e))
          .subscribe(|_| {});
      });

    source.error("boom");
    assert_eq!(*window_errors.borrow(), vec!["boom"]);
    assert_eq!(*outer_errors.borrow(), vec!["boom"]);
    assert_eq!(closing.inner.subscriber_count(), 0);
  }
}
