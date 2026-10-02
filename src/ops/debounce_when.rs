//! DebounceWhen operator implementation
//!
//! Emits an item only when the observable chosen for it emits or completes
//! before the source produces another item; a newer item cancels the
//! pending one and its duration.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

#[doc(alias = "debounce")]
#[derive(Clone)]
pub struct DebounceWhen<S, F> {
  pub source: S,
  pub selector: F,
}

impl<S, F> ObservableType for DebounceWhen<S, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

pub struct DebounceWhenState<O, Item> {
  observer: Option<O>,
  pending: Option<Item>,
  generation: usize,
}

pub struct DebounceWhenDurationObserver<StateRc> {
  state: StateRc,
  generation: usize,
}

impl<StateRc> DebounceWhenDurationObserver<StateRc> {
  fn fire<Item, Err, O>(&mut self)
  where
    StateRc: RcDerefMut<Target = DebounceWhenState<O, Item>>,
    O: Observer<Item, Err>,
  {
    let value = {
      let mut st = self.state.rc_deref_mut();
      if st.generation != self.generation {
        return;
      }
      st.pending.take()
    };
    if let Some(value) = value
      && let Some(observer) = self.state.rc_deref_mut().observer.as_mut()
    {
      observer.next(value);
    }
  }
}

impl<StateRc, Item, Err, O, DurationItem> Observer<DurationItem, Err>
  for DebounceWhenDurationObserver<StateRc>
where
  StateRc: RcDerefMut<Target = DebounceWhenState<O, Item>>,
  O: Observer<Item, Err>,
{
  fn next(&mut self, _value: DurationItem) { self.fire::<Item, Err, O>(); }

  fn error(self, err: Err) {
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(mut self) { self.fire::<Item, Err, O>(); }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.generation != self.generation
      || st.pending.is_none()
      || st.observer.as_ref().is_none_or(|o| o.is_closed())
  }
}

pub struct DebounceWhenSourceObserver<StateRc, SlotRc, F> {
  state: StateRc,
  slot: SlotRc,
  selector: F,
}

impl<StateRc, SlotRc, F, O, Item, Err, Out, BoxedSub> Observer<Item, Err>
  for DebounceWhenSourceObserver<StateRc, SlotRc, F>
where
  StateRc: RcDerefMut<Target = DebounceWhenState<O, Item>> + Clone,
  SlotRc: RcDerefMut<Target = Option<BoxedSub>>,
  BoxedSub: Subscription,
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<DebounceWhenDurationObserver<StateRc>>,
      Unsub: IntoBoxedSubscription<BoxedSub>,
    >,
  >,
{
  fn next(&mut self, value: Item) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let duration = (self.selector)(&value).into_inner();
    let generation = {
      let mut st = self.state.rc_deref_mut();
      st.generation += 1;
      st.pending = Some(value);
      st.generation
    };
    // The previous duration is stale; we are in the source's dispatch, so it
    // is safe to unsubscribe it here.
    if let Some(previous) = self.slot.rc_deref_mut().take() {
      previous.unsubscribe();
    }
    let observer = DebounceWhenDurationObserver { state: self.state.clone(), generation };
    let unsub = duration.subscribe(Out::lift(observer));
    // A duration that fired synchronously has nothing left to cancel.
    if self.state.rc_deref().pending.is_some() {
      *self.slot.rc_deref_mut() = Some(unsub.into_boxed());
    }
  }

  fn error(self, err: Err) {
    if let Some(duration) = self.slot.rc_deref_mut().take() {
      duration.unsubscribe();
    }
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    if let Some(duration) = self.slot.rc_deref_mut().take() {
      duration.unsubscribe();
    }
    let (pending, observer) = {
      let mut st = self.state.rc_deref_mut();
      st.generation += 1;
      (st.pending.take(), st.observer.take())
    };
    if let Some(mut observer) = observer {
      if let Some(value) = pending {
        observer.next(value);
      }
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
  <C as Context>::RcMut<DebounceWhenState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>;
type SlotRc<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type DwSource<'a, C, S, F> = DebounceWhenSourceObserver<StateRc<'a, C, S>, SlotRc<C>, F>;
type DwDuration<'a, C, S> = DebounceWhenDurationObserver<StateRc<'a, C, S>>;

impl<S, F, C, Out, SourceUnsub> CoreObservable<C> for DebounceWhen<S, F>
where
  C: Context,
  S: for<'a> CoreObservable<C::With<DwSource<'a, C, S, F>>, Unsub = SourceUnsub>,
  F: for<'a> FnMut(&<S as ObservableType>::Item<'a>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<DwDuration<'a, C, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>,
  SourceUnsub: Subscription,
  SlotRc<C>: Subscription,
{
  type Unsub = TupleSubscription<SourceUnsub, SlotRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DebounceWhen { source, selector } = self;
    let state: StateRc<'_, C, S> = C::RcMut::from(DebounceWhenState {
      observer: Some(context.into_inner()),
      pending: None,
      generation: 0,
    });
    let slot: SlotRc<C> = C::RcMut::from(None);
    let observer = DebounceWhenSourceObserver { state, slot: slot.clone(), selector };
    let source_unsub = source.subscribe(C::lift(observer));
    TupleSubscription::new(source_unsub, slot)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  type Durations = Rc<RefCell<Vec<LocalSubject<'static, (), Infallible>>>>;

  #[rxrust_macro::test]
  fn test_debounce_when_emits_when_duration_fires() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let durations: Durations = Rc::new(RefCell::new(Vec::new()));
    let (sink, durations_c) = (seen.clone(), durations.clone());
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .debounce_when(move |_v: &i32| {
        let duration = Local::subject::<(), Infallible>();
        durations_c.borrow_mut().push(duration.clone());
        duration
      })
      .subscribe(move |v| sink.borrow_mut().push(v));

    source.next(1);
    assert!(seen.borrow().is_empty());
    durations.borrow()[0].clone().next(());
    assert_eq!(*seen.borrow(), vec![1]);

    // A newer item releases the fired duration and cancels the pending one
    // and its duration.
    source.next(2);
    assert_eq!(durations.borrow()[0].inner.subscriber_count(), 0, "fired duration released");
    source.next(3);
    assert_eq!(durations.borrow()[1].inner.subscriber_count(), 0, "stale duration unsubscribed");
    durations.borrow()[1].clone().next(()); // ignored
    assert_eq!(*seen.borrow(), vec![1]);
    durations.borrow()[2].clone().next(());
    assert_eq!(*seen.borrow(), vec![1, 3]);
  }

  #[rxrust_macro::test]
  fn test_debounce_when_duration_completion_emits() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let duration = Local::subject::<(), Infallible>();
    let duration_c = duration.clone();

    source
      .clone()
      .debounce_when(move |_v: &i32| duration_c.clone())
      .subscribe(move |v| sink.borrow_mut().push(v));

    source.next(7);
    duration.clone().complete();
    assert_eq!(*seen.borrow(), vec![7]);
  }

  #[rxrust_macro::test]
  fn test_debounce_when_source_completion_flushes_pending() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let done = Rc::new(RefCell::new(false));
    let (sink, done_c) = (seen.clone(), done.clone());
    let mut source = Local::subject::<i32, Infallible>();
    let duration = Local::subject::<(), Infallible>();
    let duration_c = duration.clone();

    source
      .clone()
      .debounce_when(move |_v: &i32| duration_c.clone())
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(move |v| sink.borrow_mut().push(v));

    source.next(1);
    source.next(2);
    source.complete();
    assert_eq!(*seen.borrow(), vec![2]);
    assert!(*done.borrow());
    assert_eq!(duration.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_debounce_when_synchronous_duration_emits_immediately() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();

    Local::from_iter(vec![1, 2, 3])
      .debounce_when(|v: &i32| Local::of(*v))
      .subscribe(move |v| sink.borrow_mut().push(v));

    assert_eq!(*seen.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_debounce_when_duration_error_propagates() {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let errors_c = errors.clone();
    let mut source = Local::subject::<i32, &'static str>();
    let duration = Local::subject::<(), &'static str>();
    let duration_c = duration.clone();

    source
      .clone()
      .debounce_when(move |_v: &i32| duration_c.clone())
      .on_error(move |e| errors_c.borrow_mut().push(e))
      .subscribe(|_| {});

    source.next(1);
    duration.clone().error("boom");
    assert_eq!(*errors.borrow(), vec!["boom"]);
  }
}
