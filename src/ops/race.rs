//! Race operator implementation
//!
//! Mirrors whichever of two sources emits first and drops the other.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// Race operator: Mirrors the first source to emit any event
///
/// Both sources are subscribed. The first one to emit an item, error, or
/// completion wins; the other is unsubscribed and the winner is mirrored
/// from that event on. See
/// [`crate::factory::ObservableFactory::race_observables`] for the N-ary form.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 2])
///   .race(Local::from_iter([3, 4]))
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "raceWith")]
#[derive(Clone)]
pub struct Race<A, B> {
  pub source_a: A,
  pub source_b: B,
}

impl<A, B> ObservableType for Race<A, B>
where
  A: ObservableType,
{
  type Item<'a>
    = A::Item<'a>
  where
    Self: 'a;
  type Err = A::Err;
}

/// Which side of the race an observer belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceSide {
  /// The receiver of `race`
  A,
  /// The argument of `race`
  B,
}

/// State shared by both race observers
pub struct RaceState<O> {
  observer: Option<O>,
  winner: Option<RaceSide>,
}

/// Observer for one side of the race
pub struct RaceObserver<StateRc, OtherProxy> {
  state: StateRc,
  other: OtherProxy,
  side: RaceSide,
}

impl<StateRc, OtherProxy, O> RaceObserver<StateRc, OtherProxy>
where
  StateRc: RcDerefMut<Target = RaceState<O>>,
  OtherProxy: Subscription + Clone,
{
  /// Returns true when this side is, or just became, the winner. Claiming
  /// unsubscribes the other side.
  fn claim(&mut self) -> bool {
    let mut state = self.state.rc_deref_mut();
    match state.winner {
      None => {
        state.winner = Some(self.side);
        drop(state);
        self.other.clone().unsubscribe();
        true
      }
      Some(side) => side == self.side,
    }
  }
}

impl<Item, Err, O, StateRc, OtherProxy> Observer<Item, Err> for RaceObserver<StateRc, OtherProxy>
where
  O: Observer<Item, Err>,
  StateRc: RcDerefMut<Target = RaceState<O>>,
  OtherProxy: Subscription + Clone,
{
  fn next(&mut self, value: Item) {
    if self.claim() {
      let mut state = self.state.rc_deref_mut();
      if let Some(observer) = state.observer.as_mut() {
        observer.next(value);
      }
    }
  }

  fn error(mut self, err: Err) {
    if self.claim() {
      let observer = self.state.rc_deref_mut().observer.take();
      if let Some(observer) = observer {
        observer.error(err);
      }
    }
  }

  fn complete(mut self) {
    if self.claim() {
      let observer = self.state.rc_deref_mut().observer.take();
      if let Some(observer) = observer {
        observer.complete();
      }
    }
  }

  fn is_closed(&self) -> bool {
    let state = self.state.rc_deref();
    state.winner.is_some_and(|w| w != self.side)
      || state
        .observer
        .as_ref()
        .is_none_or(|o| o.is_closed())
  }
}

type StateRc<C> = <C as Context>::RcMut<RaceState<<C as Context>::Inner>>;
type BoxedProxy<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type Proxy<C, U> = <C as Context>::RcMut<Option<U>>;

impl<A, B, C, BUnsub> CoreObservable<C> for Race<A, B>
where
  C: Context,
  A: CoreObservable<C::With<RaceObserver<StateRc<C>, Proxy<C, BUnsub>>>>,
  A::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  B: CoreObservable<C::With<RaceObserver<StateRc<C>, BoxedProxy<C>>>, Unsub = BUnsub>,
  BoxedProxy<C>: Subscription,
  Proxy<C, BUnsub>: Subscription,
{
  type Unsub = TupleSubscription<BoxedProxy<C>, Proxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Race { source_a, source_b } = self;

    let state: StateRc<C> =
      C::RcMut::from(RaceState { observer: Some(context.into_inner()), winner: None });
    let a_proxy: BoxedProxy<C> = C::RcMut::from(None);
    let b_proxy: Proxy<C, BUnsub> = C::RcMut::from(None);

    let a_observer =
      RaceObserver { state: state.clone(), other: b_proxy.clone(), side: RaceSide::A };
    let a_unsub = source_a.subscribe(C::lift(a_observer));
    *a_proxy.rc_deref_mut() = Some(a_unsub.into_boxed());

    // A synchronous source may have already won; then B is never subscribed.
    if state.rc_deref().winner.is_some() {
      return TupleSubscription::new(a_proxy, b_proxy);
    }

    let b_observer =
      RaceObserver { state: state.clone(), other: a_proxy.clone(), side: RaceSide::B };
    let b_unsub = source_b.subscribe(C::lift(b_observer));
    *b_proxy.rc_deref_mut() = Some(b_unsub);

    // A may have won on another thread while B was subscribing.
    if state.rc_deref().winner == Some(RaceSide::A) {
      b_proxy.clone().unsubscribe();
    }

    TupleSubscription::new(a_proxy, b_proxy)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_race_first_side_wins() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    a.clone()
      .race(b.clone())
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    b.next(10);
    a.next(2);

    assert_eq!(*result.borrow(), vec![1, 2]);
    // The loser was unsubscribed
    assert_eq!(b.inner.subscriber_count(), 0);
    assert_eq!(a.inner.subscriber_count(), 1);
  }

  #[rxrust_macro::test]
  fn test_race_second_side_wins() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    a.clone()
      .race(b.clone())
      .subscribe(move |v| result_c.borrow_mut().push(v));

    b.next(10);
    a.next(1);
    b.next(20);

    assert_eq!(*result.borrow(), vec![10, 20]);
    assert_eq!(a.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_race_sync_source_wins_without_subscribing_other() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let b = Local::subject::<i32, Infallible>();

    Local::from_iter([1, 2])
      .race(b.clone())
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert_eq!(b.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_race_completion_counts_as_winning() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .race(Local::of(1))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_race_error_counts_as_winning() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .race(Local::of(1).map_err(|_: Infallible| String::new()))
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }

  #[rxrust_macro::test]
  fn test_race_unsubscribe_cancels_both() {
    let a = Local::subject::<i32, Infallible>();
    let b = Local::subject::<i32, Infallible>();

    let sub = a.clone().race(b.clone()).subscribe(|_| {});
    assert_eq!(a.inner.subscriber_count(), 1);
    assert_eq!(b.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert_eq!(a.inner.subscriber_count(), 0);
    assert_eq!(b.inner.subscriber_count(), 0);
  }
}
