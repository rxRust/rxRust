//! BufferWhen operator implementation
//!
//! Buffers items until a closing observable, obtained from a selector for
//! every buffer, emits.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// BufferWhen operator: Emit buffers closed by `closing_selector()`
///
/// A buffer opens at subscribe together with a closing observable from the
/// selector. When it emits, the buffer is emitted and a new buffer and
/// closing observable start. The closing observable's completion is ignored.
/// Source completion emits the last buffer. The closing observable must not
/// emit synchronously while it is being subscribed.
///
/// # Examples
///
/// ```rust,no_run
/// use rxrust::prelude::*;
///
/// # #[cfg(not(target_arch = "wasm32"))]
/// # {
/// # #[tokio::main(flavor = "local")]
/// # async fn main() {
/// Local::interval(Duration::from_millis(10))
///   .buffer_when(|| Local::timer(Duration::from_millis(100)))
///   .subscribe(|b| println!("{:?}", b));
/// # }
/// # }
/// ```
#[doc(alias = "bufferWhen")]
#[derive(Clone)]
pub struct BufferWhen<S, F> {
  pub source: S,
  pub closing_selector: F,
}

impl<S, F> ObservableType for BufferWhen<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = Vec<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// State shared by the source and closing observers
pub struct BufferWhenState<O, Item> {
  observer: Option<O>,
  buffer: Vec<Item>,
  generation: usize,
}

/// Observer for a closing observable
pub struct BufferWhenClosingObserver<StateRc, SelRc, SlotRc> {
  state: StateRc,
  selector: SelRc,
  slot: SlotRc,
  generation: usize,
  reopen: fn(StateRc, SelRc, SlotRc),
}

/// Opens the next buffer's closing observable.
fn open_closing<Out, StateRc, SelRc, SlotRc, O, Item, F, BoxedSub>(
  state: StateRc, selector: SelRc, slot: SlotRc,
) where
  StateRc: RcDerefMut<Target = BufferWhenState<O, Item>> + Clone,
  SelRc: RcDerefMut<Target = F> + Clone,
  SlotRc: RcDerefMut<Target = Option<BoxedSub>> + Clone,
  F: FnMut() -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<BufferWhenClosingObserver<StateRc, SelRc, SlotRc>>,
      Unsub: IntoBoxedSubscription<BoxedSub>,
    >,
  >,
{
  if state.rc_deref().observer.is_none() {
    return;
  }
  let generation = {
    let mut st = state.rc_deref_mut();
    st.generation += 1;
    st.generation
  };
  let closing = (selector.rc_deref_mut())().into_inner();
  let observer = BufferWhenClosingObserver {
    state: state.clone(),
    selector: selector.clone(),
    slot: slot.clone(),
    generation,
    reopen: open_closing::<Out, StateRc, SelRc, SlotRc, O, Item, F, BoxedSub>,
  };
  let unsub = closing.subscribe(Out::lift(observer));
  // A stale handle is simply replaced; the old closing sees `is_closed`.
  *slot.rc_deref_mut() = Some(unsub.into_boxed());
}

impl<StateRc, SelRc, SlotRc, O, Item, Err, NotifyItem> Observer<NotifyItem, Err>
  for BufferWhenClosingObserver<StateRc, SelRc, SlotRc>
where
  StateRc: RcDerefMut<Target = BufferWhenState<O, Item>> + Clone,
  O: Observer<Vec<Item>, Err>,
  SelRc: Clone,
  SlotRc: Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    let buffer = {
      let mut st = self.state.rc_deref_mut();
      if st.generation != self.generation || st.observer.is_none() {
        return;
      }
      std::mem::take(&mut st.buffer)
    };
    if let Some(observer) = self.state.rc_deref_mut().observer.as_mut() {
      observer.next(buffer);
    }
    (self.reopen)(self.state.clone(), self.selector.clone(), self.slot.clone());
  }

  fn error(self, err: Err) {
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    // Closing completion without a value keeps the buffer open, as in RxJS.
  }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.generation != self.generation || st.observer.as_ref().is_none_or(|o| o.is_closed())
  }
}

/// Observer for the source
pub struct BufferWhenSourceObserver<StateRc, SlotRc> {
  state: StateRc,
  slot: SlotRc,
}

impl<StateRc, SlotRc, O, Item, Err> Observer<Item, Err>
  for BufferWhenSourceObserver<StateRc, SlotRc>
where
  StateRc: RcDerefMut<Target = BufferWhenState<O, Item>>,
  O: Observer<Vec<Item>, Err>,
  SlotRc: Subscription,
{
  fn next(&mut self, value: Item) { self.state.rc_deref_mut().buffer.push(value); }

  fn error(self, err: Err) {
    self.slot.unsubscribe();
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.slot.unsubscribe();
    let mut st = self.state.rc_deref_mut();
    let buffer = std::mem::take(&mut st.buffer);
    st.generation += 1;
    if let Some(mut observer) = st.observer.take() {
      observer.next(buffer);
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

type StateRc<'a, C, S> =
  <C as Context>::RcMut<BufferWhenState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>;
type SelRc<C, F> = <C as Context>::RcMut<F>;
type SlotRc<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type BwSource<'a, C, S> = BufferWhenSourceObserver<StateRc<'a, C, S>, SlotRc<C>>;
type BwClosing<'a, C, S, F> = BufferWhenClosingObserver<StateRc<'a, C, S>, SelRc<C, F>, SlotRc<C>>;

impl<S, F, C, Out, SourceUnsub> CoreObservable<C> for BufferWhen<S, F>
where
  C: Context,
  S: for<'a> CoreObservable<C::With<BwSource<'a, C, S>>, Unsub = SourceUnsub>,
  F: FnMut() -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<BwClosing<'a, C, S, F>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: for<'a> Observer<Vec<<S as ObservableType>::Item<'a>>, S::Err>,
  SourceUnsub: Subscription,
  SlotRc<C>: Subscription,
{
  type Unsub = TupleSubscription<SourceUnsub, SlotRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let BufferWhen { source, closing_selector } = self;
    let state: StateRc<'_, C, S> = C::RcMut::from(BufferWhenState {
      observer: Some(context.into_inner()),
      buffer: Vec::new(),
      generation: 0,
    });
    let selector: SelRc<C, F> = C::RcMut::from(closing_selector);
    let slot: SlotRc<C> = C::RcMut::from(None);

    open_closing::<Out, _, _, _, C::Inner, S::Item<'_>, F, C::BoxedSubscription>(
      state.clone(),
      selector,
      slot.clone(),
    );

    let source_observer = BufferWhenSourceObserver { state, slot: slot.clone() };
    let source_unsub = source.subscribe(C::lift(source_observer));
    TupleSubscription::new(source_unsub, slot)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  #[rxrust_macro::test]
  fn test_buffer_when_closes_on_selector_emission() {
    use std::collections::VecDeque;

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut source = Local::subject::<i32, Infallible>();
    // One fresh closing subject per buffer, as a timer-based selector would
    // produce a fresh observable each time.
    let closings: Vec<_> = (0..4)
      .map(|_| Local::subject::<(), Infallible>())
      .collect();
    let queue = Rc::new(RefCell::new(closings.iter().cloned().collect::<VecDeque<_>>()));

    source
      .clone()
      .buffer_when(move || {
        queue
          .borrow_mut()
          .pop_front()
          .expect("closing available")
      })
      .subscribe(move |b| result_c.borrow_mut().push(b));

    source.next(1);
    source.next(2);
    closings[0].clone().next(());
    source.next(3);
    closings[1].clone().next(());
    closings[2].clone().next(());
    source.next(4);
    source.complete();

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3], vec![], vec![4]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_when_with_timer_on_virtual_clock() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut source = TestCtx::subject::<i32, Infallible>();

    let _sub = source
      .clone()
      .buffer_when(|| TestCtx::timer(Duration::from_millis(100)))
      .subscribe(move |b| result_c.borrow_mut().push(b));

    source.next(1);
    TestScheduler::advance_by(Duration::from_millis(100));
    source.next(2);
    source.next(3);
    TestScheduler::advance_by(Duration::from_millis(100));

    assert_eq!(*result.borrow(), vec![vec![1], vec![2, 3]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_when_error_propagation_and_cleanup() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let source = Local::subject::<i32, String>();
    let closing = Local::subject::<(), String>();
    let closing_c = closing.clone();

    source
      .clone()
      .buffer_when(move || closing_c.clone())
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});
    assert_eq!(closing.inner.subscriber_count(), 1);

    source.error("boom".to_string());
    assert_eq!(error.borrow().as_deref(), Some("boom"));
    assert_eq!(closing.inner.subscriber_count(), 0);
  }
}
