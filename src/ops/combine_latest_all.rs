//! CombineLatestAll operator implementation
//!
//! N-ary form of `combine_latest`: emits a snapshot of the latest value from
//! every source whenever any source emits.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// CombineLatestAll operator: Emits the latest values of every source
///
/// Created with
/// [`crate::factory::ObservableFactory::combine_latest_observables`]. Once
/// every source has emitted at least once, each new item from any source
/// emits a `Vec` with the latest value of every source in input order.
/// Completes when all sources complete, or as soon as a source completes
/// without ever emitting (the result can then never emit). An empty list
/// completes immediately.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::combine_latest_observables([Local::from_iter(vec![1, 2]), Local::from_iter(vec![3])])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![vec![2, 3]]);
/// ```
#[doc(alias = "combineLatest")]
pub struct CombineLatestAll<O> {
  pub sources: Vec<O>,
}

impl<O: ObservableType> ObservableType for CombineLatestAll<O> {
  type Item<'a>
    = Vec<O::Item<'a>>
  where
    Self: 'a;
  type Err = O::Err;
}

/// State shared by every combine-latest observer
pub struct CombineLatestAllState<Obs, Item> {
  observer: Option<Obs>,
  latest: Vec<Option<Item>>,
  remaining: usize,
}

/// Observer for one indexed source
pub struct CombineLatestAllObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  index: usize,
}

impl<Item, Err, Obs, StateRc, SubsRc, U> Observer<Item, Err>
  for CombineLatestAllObserver<StateRc, SubsRc>
where
  Item: Clone,
  Obs: Observer<Vec<Item>, Err>,
  StateRc: RcDerefMut<Target = CombineLatestAllState<Obs, Item>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_none() {
      return;
    }
    state.latest[self.index] = Some(value);
    if state.latest.iter().all(Option::is_some) {
      let snapshot: Vec<Item> = state.latest.iter().flatten().cloned().collect();
      if let Some(observer) = state.observer.as_mut() {
        observer.next(snapshot);
      }
    }
  }

  fn error(self, err: Err) {
    // Our own source has terminated; drop its handle rather than
    // unsubscribing it mid-dispatch, then cancel the others.
    self.subs.rc_deref_mut().remove(self.index);
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(self) {
    self.subs.rc_deref_mut().remove(self.index);
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_none() {
      return;
    }
    let never_emitted = state.latest[self.index].is_none();
    state.remaining -= 1;
    if never_emitted || state.remaining == 0 {
      let observer = state.observer.take();
      drop(state);
      if let Some(observer) = observer {
        observer.complete();
      }
      self.subs.rc_deref_mut().unsubscribe_all();
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

type StateRc<'a, C, O> = <C as Context>::RcMut<
  CombineLatestAllState<<C as Context>::Inner, <O as ObservableType>::Item<'a>>,
>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

impl<O, C> CoreObservable<C> for CombineLatestAll<O>
where
  C: Context,
  C::Inner: for<'a> Observer<Vec<O::Item<'a>>, O::Err>,
  O: ObservableType
    + for<'a> CoreObservable<
      C::With<CombineLatestAllObserver<StateRc<'a, C, O>, SubsRc<C>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
{
  type Unsub = SourceWithDynamicSubs<(), SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let total = self.sources.len();
    let state: StateRc<C, O> = C::RcMut::from(CombineLatestAllState {
      observer: Some(context.into_inner()),
      latest: (0..total).map(|_| None).collect(),
      remaining: total,
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());

    if total == 0 {
      let observer = state.rc_deref_mut().observer.take();
      if let Some(observer) = observer {
        observer.complete();
      }
      return SourceWithDynamicSubs::new((), subs);
    }

    for source in self.sources {
      if state.rc_deref().observer.is_none() {
        break;
      }
      let id = subs.rc_deref_mut().reserve_id();
      let observer =
        CombineLatestAllObserver { state: state.clone(), subs: subs.clone(), index: id };
      let unsub = source.subscribe(C::lift(observer)).into_boxed();
      if state.rc_deref().observer.is_none() {
        unsub.unsubscribe();
      } else {
        subs.rc_deref_mut().insert(id, unsub);
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
  fn test_combine_latest_observables_interleaved() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();
    let mut c = Local::subject::<i32, Infallible>();

    Local::combine_latest_observables([a.clone(), b.clone(), c.clone()])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    b.next(10);
    assert!(result.borrow().is_empty());
    c.next(100);
    a.next(2);
    b.next(20);

    assert_eq!(*result.borrow(), vec![vec![1, 10, 100], vec![2, 10, 100], vec![2, 20, 100]]);
  }

  #[rxrust_macro::test]
  fn test_combine_latest_observables_sync_sources() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::combine_latest_observables([Local::from_iter(vec![1, 2]), Local::from_iter(vec![3, 4])])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    // The first source finishes before the second subscribes
    assert_eq!(*result.borrow(), vec![vec![2, 3], vec![2, 4]]);
  }

  #[rxrust_macro::test]
  fn test_combine_latest_observables_completes_when_all_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    Local::combine_latest_observables([a.clone(), b.clone()])
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    a.next(1);
    b.next(2);
    a.complete();
    assert!(!*completed.borrow());
    b.complete();
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_combine_latest_observables_silent_source_completes_early() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    let a = Local::subject::<i32, Infallible>();
    let b = Local::subject::<i32, Infallible>();

    Local::combine_latest_observables([a.clone(), b.clone()])
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    a.clone().complete();

    assert!(*completed.borrow());
    assert_eq!(b.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_combine_latest_observables_empty_list_completes() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    Local::combine_latest_observables(std::iter::empty::<Local<Of<i32>>>())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_combine_latest_observables_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_c = result.clone();

    Shared::combine_latest_observables([Shared::from_iter(vec![1, 2]), Shared::from_iter(vec![3])])
      .subscribe(move |v| result_c.lock().unwrap().push(v));

    assert_eq!(*result.lock().unwrap(), vec![vec![2, 3]]);
  }
}
