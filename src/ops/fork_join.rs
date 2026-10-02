//! ForkJoin operator implementation
//!
//! Waits for every source to complete, then emits their last values.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// ForkJoin operator: Emits the last value of every source once all complete
///
/// Created with [`crate::factory::ObservableFactory::fork_join_observables`].
/// Emits one `Vec` holding each source's last value in input order, then
/// completes. If any source completes without emitting, the result completes
/// without emitting. An error from any source is forwarded immediately and
/// the rest are unsubscribed. An empty list completes immediately.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::fork_join_observables([Local::from_iter(vec![1, 2]), Local::from_iter(vec![3])])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![vec![2, 3]]);
/// ```
#[doc(alias = "forkJoin")]
pub struct ForkJoin<O> {
  pub sources: Vec<O>,
}

impl<O: ObservableType> ObservableType for ForkJoin<O> {
  type Item<'a>
    = Vec<O::Item<'a>>
  where
    Self: 'a;
  type Err = O::Err;
}

/// State shared by every fork-join observer
pub struct ForkJoinState<Obs, Item> {
  observer: Option<Obs>,
  values: Vec<Option<Item>>,
  remaining: usize,
}

impl<Obs, Item> ForkJoinState<Obs, Item> {
  fn finish_if_done<Err>(&mut self)
  where
    Obs: Observer<Vec<Item>, Err>,
  {
    if self.remaining != 0 {
      return;
    }
    if let Some(mut observer) = self.observer.take() {
      if self.values.iter().all(Option::is_some) {
        let values = self.values.drain(..).flatten().collect();
        observer.next(values);
      }
      observer.complete();
    }
  }
}

/// Observer for one indexed source
pub struct ForkJoinObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  index: usize,
}

impl<Item, Err, Obs, StateRc, SubsRc, U> Observer<Item, Err> for ForkJoinObserver<StateRc, SubsRc>
where
  Obs: Observer<Vec<Item>, Err>,
  StateRc: RcDerefMut<Target = ForkJoinState<Obs, Item>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_some() {
      state.values[self.index] = Some(value);
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
    if state.values[self.index].is_none() {
      // This source can never contribute, so the join can never emit.
      let observer = state.observer.take();
      drop(state);
      if let Some(observer) = observer {
        observer.complete();
      }
      self.subs.rc_deref_mut().unsubscribe_all();
      return;
    }
    state.remaining -= 1;
    state.finish_if_done::<Err>();
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

type StateRc<'a, C, O> =
  <C as Context>::RcMut<ForkJoinState<<C as Context>::Inner, <O as ObservableType>::Item<'a>>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

impl<O, C> CoreObservable<C> for ForkJoin<O>
where
  C: Context,
  C::Inner: for<'a> Observer<Vec<O::Item<'a>>, O::Err>,
  O: ObservableType
    + for<'a> CoreObservable<
      C::With<ForkJoinObserver<StateRc<'a, C, O>, SubsRc<C>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
{
  type Unsub = SourceWithDynamicSubs<(), SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let total = self.sources.len();
    let state: StateRc<C, O> = C::RcMut::from(ForkJoinState {
      observer: Some(context.into_inner()),
      values: (0..total).map(|_| None).collect(),
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
      let observer = ForkJoinObserver { state: state.clone(), subs: subs.clone(), index: id };
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
  fn test_fork_join_last_values_in_order() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::fork_join_observables([
      Local::from_iter(vec![1, 2]),
      Local::from_iter(vec![3]),
      Local::from_iter(vec![4, 5, 6]),
    ])
    .on_complete(move || *completed_c.borrow_mut() = true)
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![vec![2, 3, 6]]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_fork_join_waits_for_async_sources() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    Local::fork_join_observables([a.clone(), b.clone()])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    b.next(10);
    a.next(2);
    a.clone().complete();
    assert!(result.borrow().is_empty());

    b.next(20);
    b.complete();
    assert_eq!(*result.borrow(), vec![vec![2, 20]]);
  }

  #[rxrust_macro::test]
  fn test_fork_join_empty_source_completes_without_value() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::fork_join_observables([Local::from_iter(vec![1]), Local::from_iter(Vec::<i32>::new())])
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<Vec<i32>>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_fork_join_empty_list_completes() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    Local::fork_join_observables(std::iter::empty::<Local<Of<i32>>>())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_fork_join_error_unsubscribes_rest() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let a = Local::subject::<i32, String>();
    let b = Local::subject::<i32, String>();

    Local::fork_join_observables([a.clone(), b.clone()])
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    a.error("boom".to_string());

    assert_eq!(error.borrow().as_deref(), Some("boom"));
    assert_eq!(b.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_fork_join_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_c = result.clone();

    Shared::fork_join_observables([Shared::from_iter(vec![1, 2]), Shared::from_iter(vec![3])])
      .subscribe(move |v| result_c.lock().unwrap().push(v));

    assert_eq!(*result.lock().unwrap(), vec![vec![2, 3]]);
  }
}
