//! BufferToggle operator implementation
//!
//! Opens a buffer for every item of an `openings` observable and closes each
//! with its own closing observable; buffers may overlap.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
    TupleSubscription,
  },
};

/// BufferToggle operator: Overlapping buffers driven by openings and closings
///
/// Every item from `openings` starts a buffer, closed when
/// `closing_selector(opening_item)` emits or completes. Source items go into
/// every open buffer, so items must be `Clone`. Source completion emits the
/// open buffers in opening order.
///
/// # Examples
///
/// ```rust
/// use std::{cell::RefCell, convert::Infallible, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let result = Rc::new(RefCell::new(Vec::new()));
/// let sink = result.clone();
/// let mut source = Local::subject::<i32, Infallible>();
/// let mut openings = Local::subject::<(), Infallible>();
/// let mut closing = Local::subject::<(), Infallible>();
/// let closing_c = closing.clone();
///
/// source
///   .clone()
///   .buffer_toggle(openings.clone(), move |_| closing_c.clone())
///   .subscribe(move |b| sink.borrow_mut().push(b));
///
/// source.next(0); // no buffer open yet
/// openings.next(());
/// source.next(1);
/// source.next(2);
/// closing.next(());
/// source.complete();
/// assert_eq!(*result.borrow(), vec![vec![1, 2]]);
/// ```
#[doc(alias = "bufferToggle")]
#[derive(Clone)]
pub struct BufferToggle<S, Op, F> {
  pub source: S,
  pub openings: Op,
  pub closing_selector: F,
}

impl<S, Op, F> ObservableType for BufferToggle<S, Op, F>
where
  S: ObservableType,
{
  type Item<'a>
    = Vec<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// State shared by every observer of the operator
pub struct BufferToggleState<O, Item> {
  observer: Option<O>,
  buffers: Vec<(usize, Vec<Item>)>,
}

impl<O, Item> BufferToggleState<O, Item> {
  fn close<Err>(&mut self, id: usize)
  where
    O: Observer<Vec<Item>, Err>,
  {
    let Some(pos) = self
      .buffers
      .iter()
      .position(|(bid, _)| *bid == id)
    else {
      return;
    };
    let (_, buffer) = self.buffers.remove(pos);
    if let Some(observer) = self.observer.as_mut() {
      observer.next(buffer);
    }
  }
}

/// Observer for one closing observable
pub struct ToggleClosingObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  id: usize,
}

impl<StateRc, SubsRc, O, Item, Err, NotifyItem, U> Observer<NotifyItem, Err>
  for ToggleClosingObserver<StateRc, SubsRc>
where
  StateRc: RcDerefMut<Target = BufferToggleState<O, Item>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<Vec<Item>, Err>,
  U: Subscription,
{
  fn next(&mut self, _value: NotifyItem) {
    self.state.rc_deref_mut().close::<Err>(self.id);
    // Drop our own handle rather than unsubscribing mid-dispatch.
    self.subs.rc_deref_mut().remove(self.id);
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
    self.state.rc_deref_mut().close::<Err>(self.id);
    self.subs.rc_deref_mut().remove(self.id);
  }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.observer.as_ref().is_none_or(|o| o.is_closed())
      || !st.buffers.iter().any(|(bid, _)| *bid == self.id)
  }
}

/// Observer for the openings observable
pub struct ToggleOpeningsObserver<StateRc, SubsRc, F> {
  state: StateRc,
  subs: SubsRc,
  closing_selector: F,
}

impl<StateRc, SubsRc, F, O, Item, Err, OpenItem, Out, U> Observer<OpenItem, Err>
  for ToggleOpeningsObserver<StateRc, SubsRc, F>
where
  StateRc: RcDerefMut<Target = BufferToggleState<O, Item>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  O: Observer<Vec<Item>, Err>,
  F: FnMut(OpenItem) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<ToggleClosingObserver<StateRc, SubsRc>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
{
  fn next(&mut self, opening: OpenItem) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let id = self.subs.rc_deref_mut().reserve_id();
    self
      .state
      .rc_deref_mut()
      .buffers
      .push((id, Vec::new()));
    let closing = (self.closing_selector)(opening).into_inner();
    let observer = ToggleClosingObserver { state: self.state.clone(), subs: self.subs.clone(), id };
    let unsub = closing
      .subscribe(Out::lift(observer))
      .into_boxed();
    let still_open = self
      .state
      .rc_deref()
      .buffers
      .iter()
      .any(|(bid, _)| *bid == id);
    if still_open {
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
    // Openings ending does not end the buffers already open.
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
pub struct ToggleSourceObserver<StateRc, SubsRc, OpUnsub> {
  state: StateRc,
  subs: SubsRc,
  openings_unsub: OpUnsub,
}

impl<StateRc, SubsRc, OpUnsub, O, Item, Err, U> Observer<Item, Err>
  for ToggleSourceObserver<StateRc, SubsRc, OpUnsub>
where
  StateRc: RcDerefMut<Target = BufferToggleState<O, Item>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<Vec<Item>, Err>,
  OpUnsub: Subscription,
  U: Subscription,
  Item: Clone,
{
  fn next(&mut self, value: Item) {
    let mut st = self.state.rc_deref_mut();
    for (_, buffer) in st.buffers.iter_mut() {
      buffer.push(value.clone());
    }
  }

  fn error(self, err: Err) {
    self.openings_unsub.unsubscribe();
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    self.openings_unsub.unsubscribe();
    let (buffers, observer) = {
      let mut st = self.state.rc_deref_mut();
      (std::mem::take(&mut st.buffers), st.observer.take())
    };
    if let Some(mut observer) = observer {
      for (_, buffer) in buffers {
        observer.next(buffer);
      }
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

type StateRc<'a, C, S> =
  <C as Context>::RcMut<BufferToggleState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;
type OpProxy<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type TgSource<'a, C, S> = ToggleSourceObserver<StateRc<'a, C, S>, SubsRc<C>, OpProxy<C>>;
type TgOpenings<'a, C, S, F> = ToggleOpeningsObserver<StateRc<'a, C, S>, SubsRc<C>, F>;
type TgClosing<'a, C, S> = ToggleClosingObserver<StateRc<'a, C, S>, SubsRc<C>>;

impl<S, Op, F, C, Out, SourceUnsub> CoreObservable<C> for BufferToggle<S, Op, F>
where
  C: Context,
  S: for<'a> CoreObservable<C::With<TgSource<'a, C, S>>, Unsub = SourceUnsub>,
  Op: ObservableType<Err = S::Err>
    + for<'a> CoreObservable<
      C::With<TgOpenings<'a, C, S, F>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  F: for<'b> FnMut(<Op as ObservableType>::Item<'b>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<TgClosing<'a, C, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: for<'a> Observer<Vec<<S as ObservableType>::Item<'a>>, S::Err>,
  for<'a> <S as ObservableType>::Item<'a>: Clone,
  SourceUnsub: Subscription,
  OpProxy<C>: Subscription,
{
  type Unsub = TupleSubscription<SourceWithDynamicSubs<SourceUnsub, SubsRc<C>>, OpProxy<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let BufferToggle { source, openings, closing_selector } = self;
    let state: StateRc<'_, C, S> = C::RcMut::from(BufferToggleState {
      observer: Some(context.into_inner()),
      buffers: Vec::new(),
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let op_proxy: OpProxy<C> = C::RcMut::from(None);

    let openings_observer =
      ToggleOpeningsObserver { state: state.clone(), subs: subs.clone(), closing_selector };
    let op_unsub = openings
      .subscribe(C::lift(openings_observer))
      .into_boxed();
    *op_proxy.rc_deref_mut() = Some(op_unsub);

    let source_observer =
      ToggleSourceObserver { state, subs: subs.clone(), openings_unsub: op_proxy.clone() };
    let source_unsub = source.subscribe(C::lift(source_observer));

    TupleSubscription::new(SourceWithDynamicSubs::new(source_unsub, subs), op_proxy)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_buffer_toggle_overlapping_buffers() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<u8, Infallible>();
    let mut close_a = Local::subject::<(), Infallible>();
    let mut close_b = Local::subject::<(), Infallible>();
    let (ca, cb) = (close_a.clone(), close_b.clone());

    source
      .clone()
      .buffer_toggle(
        openings.clone(),
        move |which| if which == 0 { ca.clone() } else { cb.clone() },
      )
      .subscribe(move |b| result_c.borrow_mut().push(b));

    source.next(0);
    openings.next(0);
    source.next(1);
    openings.next(1);
    source.next(2);
    close_a.next(());
    source.next(3);
    close_b.next(());
    source.next(4);
    source.complete();

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![2, 3]]);
    assert_eq!(openings.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_buffer_toggle_flushes_open_buffers_on_completion() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<(), Infallible>();
    let closing = Local::subject::<(), Infallible>();
    let closing_c = closing.clone();

    source
      .clone()
      .buffer_toggle(openings.clone(), move |_| closing_c.clone())
      .subscribe(move |b| result_c.borrow_mut().push(b));

    openings.next(());
    source.next(1);
    openings.next(());
    source.next(2);
    source.complete();

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![2]]);
    assert_eq!(closing.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_buffer_toggle_closing_completion_closes_buffer() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let mut openings = Local::subject::<(), Infallible>();

    source
      .clone()
      .buffer_toggle(openings.clone(), |_| Local::from_iter(Vec::<()>::new()))
      .subscribe(move |b| result_c.borrow_mut().push(b));

    openings.next(());
    source.next(1);

    // The closing completed synchronously, so the buffer closed empty
    assert_eq!(*result.borrow(), vec![Vec::<i32>::new()]);
  }

  #[rxrust_macro::test]
  fn test_buffer_toggle_error_propagation() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let source = Local::subject::<i32, String>();
    let openings = Local::subject::<(), String>();

    source
      .clone()
      .buffer_toggle(openings.clone(), |_| Local::of(()).map_err(|_: Infallible| String::new()))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    source.error("boom".to_string());
    assert_eq!(error.borrow().as_deref(), Some("boom"));
    assert_eq!(openings.inner.subscriber_count(), 0);
  }
}
