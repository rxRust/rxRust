//! RaceAll operator implementation
//!
//! N-ary form of `race`: mirrors the first of many sources to emit.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// RaceAll operator: Mirrors the first of many sources to emit any event
///
/// Created with [`crate::factory::ObservableFactory::race_observables`].
/// Sources are subscribed in order; once one emits, the rest are
/// unsubscribed (or never subscribed) and the winner is mirrored. An empty
/// list completes immediately.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::race_observables([Local::from_iter(vec![1, 2]), Local::from_iter(vec![3])])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "race")]
pub struct RaceAll<O> {
  pub sources: Vec<O>,
}

impl<O: ObservableType> ObservableType for RaceAll<O> {
  type Item<'a>
    = O::Item<'a>
  where
    Self: 'a;
  type Err = O::Err;
}

/// State shared by every race observer
pub struct RaceAllState<Obs> {
  observer: Option<Obs>,
  winner: Option<usize>,
}

/// Observer for one indexed source
pub struct RaceAllObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  index: usize,
}

impl<StateRc, SubsRc, Obs, U> RaceAllObserver<StateRc, SubsRc>
where
  StateRc: RcDerefMut<Target = RaceAllState<Obs>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
{
  /// Returns true when this source is, or just became, the winner. Claiming
  /// unsubscribes every other source and keeps our own subscription so the
  /// downstream can still cancel it.
  fn claim(&mut self) -> bool {
    let mut state = self.state.rc_deref_mut();
    match state.winner {
      None => {
        state.winner = Some(self.index);
        drop(state);
        let mut subs = self.subs.rc_deref_mut();
        let own = subs.remove(self.index);
        subs.unsubscribe_all();
        if let Some(own) = own {
          subs.insert(self.index, own);
        }
        true
      }
      Some(w) => w == self.index,
    }
  }
}

impl<Item, Err, Obs, StateRc, SubsRc, U> Observer<Item, Err> for RaceAllObserver<StateRc, SubsRc>
where
  Obs: Observer<Item, Err>,
  StateRc: RcDerefMut<Target = RaceAllState<Obs>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
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
    state.winner.is_some_and(|w| w != self.index)
      || state
        .observer
        .as_ref()
        .is_none_or(|o| o.is_closed())
  }
}

type StateRc<C> = <C as Context>::RcMut<RaceAllState<<C as Context>::Inner>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

impl<O, C> CoreObservable<C> for RaceAll<O>
where
  C: Context,
  C::Inner: for<'a> Observer<O::Item<'a>, O::Err>,
  O: ObservableType
    + CoreObservable<
      C::With<RaceAllObserver<StateRc<C>, SubsRc<C>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
{
  type Unsub = SourceWithDynamicSubs<(), SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let state: StateRc<C> =
      C::RcMut::from(RaceAllState { observer: Some(context.into_inner()), winner: None });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());

    let mut subscribed_any = false;
    for source in self.sources {
      if state.rc_deref().winner.is_some() {
        break;
      }
      subscribed_any = true;
      let id = subs.rc_deref_mut().reserve_id();
      let observer = RaceAllObserver { state: state.clone(), subs: subs.clone(), index: id };
      let unsub = source.subscribe(C::lift(observer)).into_boxed();
      let winner = state.rc_deref().winner;
      match winner {
        Some(w) if w != id => unsub.unsubscribe(),
        _ => subs.rc_deref_mut().insert(id, unsub),
      }
    }

    if !subscribed_any {
      let observer = state.rc_deref_mut().observer.take();
      if let Some(observer) = observer {
        observer.complete();
      }
    }

    SourceWithDynamicSubs::new((), subs)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::RefCell,
    convert::Infallible,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_race_observables_sync_first_wins() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::race_observables([
      Local::from_iter(vec![1, 2]),
      Local::from_iter(vec![3, 4]),
      Local::from_iter(vec![5]),
    ])
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_race_observables_later_source_wins() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();
    let mut c = Local::subject::<i32, Infallible>();

    Local::race_observables([a.clone(), b.clone(), c.clone()])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    c.next(30);
    a.next(1);
    b.next(2);
    c.next(31);

    assert_eq!(*result.borrow(), vec![30, 31]);
    assert_eq!(a.inner.subscriber_count(), 0);
    assert_eq!(b.inner.subscriber_count(), 0);
    assert_eq!(c.inner.subscriber_count(), 1);
  }

  #[rxrust_macro::test]
  fn test_race_observables_empty_list_completes() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    Local::race_observables(std::iter::empty::<Local<Of<i32>>>())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_race_observables_unsubscribe_cancels_all() {
    let a = Local::subject::<i32, Infallible>();
    let b = Local::subject::<i32, Infallible>();

    let sub = Local::race_observables([a.clone(), b.clone()]).subscribe(|_| {});
    assert_eq!(a.inner.subscriber_count(), 1);
    assert_eq!(b.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert_eq!(a.inner.subscriber_count(), 0);
    assert_eq!(b.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_race_observables_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_c = result.clone();

    Shared::race_observables([Shared::from_iter(vec![1, 2]), Shared::from_iter(vec![3])])
      .subscribe(move |v| result_c.lock().unwrap().push(v));

    assert_eq!(*result.lock().unwrap(), vec![1, 2]);
  }
}
