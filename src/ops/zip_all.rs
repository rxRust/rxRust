//! ZipAll operator implementation
//!
//! N-ary form of `zip`: emits the nth item of every source as one `Vec`.

use std::collections::VecDeque;

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

/// ZipAll operator: Pairs the nth items of every source
///
/// Created with [`crate::factory::ObservableFactory::zip_observables`].
/// Buffers items per source and emits a `Vec` in input order each time every
/// source has an item waiting. Completes when a completed source has no
/// buffered items left, since no further row can be formed. An empty list
/// completes immediately.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::zip_observables([Local::from_iter(vec![1, 2, 3]), Local::from_iter(vec![10, 20])])
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![vec![1, 10], vec![2, 20]]);
/// ```
#[doc(alias = "zip")]
pub struct ZipAll<O> {
  pub sources: Vec<O>,
}

impl<O: ObservableType> ObservableType for ZipAll<O> {
  type Item<'a>
    = Vec<O::Item<'a>>
  where
    Self: 'a;
  type Err = O::Err;
}

/// State shared by every zip observer
pub struct ZipAllState<Obs, Item> {
  observer: Option<Obs>,
  buffers: Vec<VecDeque<Item>>,
  completed: Vec<bool>,
}

impl<Obs, Item> ZipAllState<Obs, Item> {
  /// A completed source with an empty buffer means no more rows can form.
  fn exhausted(&self) -> bool {
    self
      .buffers
      .iter()
      .zip(&self.completed)
      .any(|(buffer, done)| *done && buffer.is_empty())
  }
}

/// Observer for one indexed source
pub struct ZipAllObserver<StateRc, SubsRc> {
  state: StateRc,
  subs: SubsRc,
  index: usize,
}

impl<Item, Err, Obs, StateRc, SubsRc, U> Observer<Item, Err> for ZipAllObserver<StateRc, SubsRc>
where
  Obs: Observer<Vec<Item>, Err>,
  StateRc: RcDerefMut<Target = ZipAllState<Obs, Item>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  U: Subscription,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    if state.observer.is_none() {
      return;
    }
    state.buffers[self.index].push_back(value);
    if state.buffers.iter().all(|b| !b.is_empty()) {
      let row: Vec<Item> = state
        .buffers
        .iter_mut()
        .map(|b| b.pop_front().expect("checked non-empty"))
        .collect();
      if let Some(observer) = state.observer.as_mut() {
        observer.next(row);
      }
      if state.exhausted() {
        let observer = state.observer.take();
        drop(state);
        if let Some(observer) = observer {
          observer.complete();
        }
        // We are inside our own source's dispatch: drop its handle and let
        // `is_closed` end it, then cancel the others.
        let mut subs = self.subs.rc_deref_mut();
        subs.remove(self.index);
        subs.unsubscribe_all();
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
    state.completed[self.index] = true;
    if state.exhausted() {
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

type StateRc<'a, C, O> =
  <C as Context>::RcMut<ZipAllState<<C as Context>::Inner, <O as ObservableType>::Item<'a>>>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

impl<O, C> CoreObservable<C> for ZipAll<O>
where
  C: Context,
  C::Inner: for<'a> Observer<Vec<O::Item<'a>>, O::Err>,
  O: ObservableType
    + for<'a> CoreObservable<
      C::With<ZipAllObserver<StateRc<'a, C, O>, SubsRc<C>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
{
  type Unsub = SourceWithDynamicSubs<(), SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let total = self.sources.len();
    let state: StateRc<C, O> = C::RcMut::from(ZipAllState {
      observer: Some(context.into_inner()),
      buffers: (0..total).map(|_| VecDeque::new()).collect(),
      completed: vec![false; total],
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
      let observer = ZipAllObserver { state: state.clone(), subs: subs.clone(), index: id };
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
  fn test_zip_observables_sync_sources() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::zip_observables([
      Local::from_iter(vec![1, 2, 3]),
      Local::from_iter(vec![10, 20]),
      Local::from_iter(vec![100, 200, 300]),
    ])
    .on_complete(move || *completed_c.borrow_mut() = true)
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![vec![1, 10, 100], vec![2, 20, 200]]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_zip_observables_interleaved() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    Local::zip_observables([a.clone(), b.clone()])
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    a.next(2);
    assert!(result.borrow().is_empty());
    b.next(10);
    assert_eq!(*result.borrow(), vec![vec![1, 10]]);
    b.next(20);
    assert_eq!(*result.borrow(), vec![vec![1, 10], vec![2, 20]]);
  }

  #[rxrust_macro::test]
  fn test_zip_observables_completes_when_a_source_is_exhausted() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    Local::zip_observables([a.clone(), b.clone()])
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    a.clone().complete();
    // `a` still has a buffered item, so a row could still form
    assert!(!*completed.borrow());
    assert_eq!(b.inner.subscriber_count(), 1);

    b.next(10);
    // Row emitted, `a` is now exhausted, so the zip completed
    assert_eq!(*result.borrow(), vec![vec![1, 10]]);
    assert!(*completed.borrow());

    // The surviving source is closed to us; further items are ignored
    b.next(20);
    assert_eq!(*result.borrow(), vec![vec![1, 10]]);
  }

  #[rxrust_macro::test]
  fn test_zip_observables_empty_list_completes() {
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    Local::zip_observables(std::iter::empty::<Local<Of<i32>>>())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_zip_observables_error_propagation() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let a = Local::subject::<i32, String>();
    let b = Local::subject::<i32, String>();

    Local::zip_observables([a.clone(), b.clone()])
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    a.error("boom".to_string());

    assert_eq!(error.borrow().as_deref(), Some("boom"));
    assert_eq!(b.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_zip_observables_shared() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_c = result.clone();

    Shared::zip_observables([Shared::from_iter(vec![1, 2]), Shared::from_iter(vec![3, 4])])
      .subscribe(move |v| result_c.lock().unwrap().push(v));

    assert_eq!(*result.lock().unwrap(), vec![vec![1, 3], vec![2, 4]]);
  }
}
