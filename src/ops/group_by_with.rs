//! GroupBy with a duration selector and a connector (RxJS `groupBy` options).
//!
//! Each key gets a subject created by a [`Connector`]; the observable
//! returned by the duration selector for a group closes it (completes its
//! subject) when it emits or completes, and the next item with that key opens
//! a new group.

use std::{collections::HashMap, hash::Hash, marker::PhantomData};

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  ops::{group_by::GroupedObservable, share::Connector},
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SourceWithDynamicSubs, Subscription,
  },
};

#[doc(alias = "groupBy")]
pub struct GroupByWith<S, F, Conn, D, CtxMarker> {
  pub source: S,
  pub key_selector: F,
  pub connector: Conn,
  pub duration: D,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<S: Clone, F: Clone, Conn: Clone, D: Clone, CtxMarker> Clone
  for GroupByWith<S, F, Conn, D, CtxMarker>
{
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      key_selector: self.key_selector.clone(),
      connector: self.connector.clone(),
      duration: self.duration.clone(),
      _marker: PhantomData,
    }
  }
}

impl<S, F, Conn, D, CtxMarker> GroupByWith<S, F, Conn, D, CtxMarker> {
  pub fn new(source: S, key_selector: F, connector: Conn, duration: D) -> Self {
    Self { source, key_selector, connector, duration, _marker: PhantomData }
  }
}

impl<S, F, Key, Conn, D, CtxMarker> ObservableType for GroupByWith<S, F, Conn, D, CtxMarker>
where
  S: ObservableType,
  CtxMarker: Context,
  F: for<'a> FnMut(&S::Item<'a>) -> Key,
{
  type Item<'m>
    = CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>
  where
    Self: 'm;
  type Err = S::Err;
}

pub struct GroupByWithState<O, Key, Subj> {
  observer: Option<O>,
  groups: HashMap<Key, (usize, Subj)>,
  /// Set while a group subject dispatches an item: a duration that fires
  /// re-entrantly queues its close instead of completing mid-dispatch.
  dispatching: bool,
  pending_close: Vec<Key>,
}

impl<O, Key: Hash + Eq, Subj> GroupByWithState<O, Key, Subj> {
  fn take_all(&mut self) -> (Vec<Subj>, Option<O>) {
    let groups = self.groups.drain().map(|(_, (_, s))| s).collect();
    (groups, self.observer.take())
  }

  fn is_current(&self, key: &Key, id: usize) -> bool {
    self
      .groups
      .get(key)
      .is_some_and(|(gid, _)| *gid == id)
  }
}

/// Observer for one group's duration observable
pub struct GroupDurationObserver<StateRc, SubsRc, Key, CtxMarker, Item> {
  state: StateRc,
  subs: SubsRc,
  key: Key,
  id: usize,
  _marker: PhantomData<fn() -> (CtxMarker, Item)>,
}

impl<StateRc, SubsRc, Key, CtxMarker, Item, O, Err, NotifyItem, U> Observer<NotifyItem, Err>
  for GroupDurationObserver<StateRc, SubsRc, Key, CtxMarker, Item>
where
  CtxMarker: Context<Inner: Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = GroupByWithState<O, Key, CtxMarker::Inner>>,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>>,
  O: Observer<CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>, Err>,
  Key: Hash + Eq + Clone,
  U: Subscription,
  Err: Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    let closing = {
      let mut st = self.state.rc_deref_mut();
      if !st.is_current(&self.key, self.id) {
        None
      } else if st.dispatching {
        st.pending_close.push(self.key.clone());
        None
      } else {
        st.groups
          .remove(&self.key)
          .map(|(_, subject)| subject)
      }
    };
    if let Some(subject) = closing {
      subject.complete();
    }
    // Drop our own handle rather than unsubscribing mid-dispatch.
    self.subs.rc_deref_mut().remove(self.id);
  }

  fn error(self, err: Err) {
    self.subs.rc_deref_mut().remove(self.id);
    let (groups, observer) = self.state.rc_deref_mut().take_all();
    for group in groups {
      group.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn complete(mut self) { self.next(()); }

  fn is_closed(&self) -> bool {
    let st = self.state.rc_deref();
    st.observer.as_ref().is_none_or(|o| o.is_closed()) || !st.is_current(&self.key, self.id)
  }
}

/// Observer for the source
pub struct GroupByWithSourceObserver<StateRc, SubsRc, F, Conn, D, CtxMarker> {
  state: StateRc,
  subs: SubsRc,
  key_selector: F,
  connector: Conn,
  duration: D,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<StateRc, SubsRc, F, Conn, D, CtxMarker, Key, Item, Err, O, Out, U> Observer<Item, Err>
  for GroupByWithSourceObserver<StateRc, SubsRc, F, Conn, D, CtxMarker>
where
  CtxMarker: Context<Inner: Clone + Observer<Item, Err>>,
  StateRc: RcDerefMut<Target = GroupByWithState<O, Key, CtxMarker::Inner>> + Clone,
  SubsRc: RcDerefMut<Target = DynamicSubscriptions<U>> + Clone,
  O: Observer<CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>, Err>,
  F: FnMut(&Item) -> Key,
  Key: Hash + Eq + Clone,
  Conn: Connector<Subject = CtxMarker::Inner>,
  D: FnMut(CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>) -> Out,
  Out: Context<
    Inner: CoreObservable<
      Out::With<GroupDurationObserver<StateRc, SubsRc, Key, CtxMarker, Item>>,
      Unsub: IntoBoxedSubscription<U>,
    >,
  >,
  U: Subscription,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    if self.state.rc_deref().observer.is_none() {
      return;
    }
    let key = (self.key_selector)(&value);
    let existing = self
      .state
      .rc_deref()
      .groups
      .get(&key)
      .map(|(_, subject)| subject.clone());

    let mut subject = match existing {
      Some(subject) => subject,
      None => {
        let id = self.subs.rc_deref_mut().reserve_id();
        let subject = self.connector.create();
        self
          .state
          .rc_deref_mut()
          .groups
          .insert(key.clone(), (id, subject.clone()));
        let grouped = |subject: &CtxMarker::Inner| {
          CtxMarker::lift(GroupedObservable { key: key.clone(), subject: subject.clone() })
        };
        let duration = (self.duration)(grouped(&subject)).into_inner();
        if let Some(observer) = self.state.rc_deref_mut().observer.as_mut() {
          observer.next(grouped(&subject));
        }
        let duration_observer = GroupDurationObserver {
          state: self.state.clone(),
          subs: self.subs.clone(),
          key: key.clone(),
          id,
          _marker: PhantomData,
        };
        let unsub = duration
          .subscribe(Out::lift(duration_observer))
          .into_boxed();
        if self.state.rc_deref().is_current(&key, id) {
          self.subs.rc_deref_mut().insert(id, unsub);
        }
        subject
      }
    };

    self.state.rc_deref_mut().dispatching = true;
    subject.next(value);
    let closing: Vec<CtxMarker::Inner> = {
      let mut st = self.state.rc_deref_mut();
      st.dispatching = false;
      let pending = std::mem::take(&mut st.pending_close);
      pending
        .into_iter()
        .filter_map(|key| st.groups.remove(&key).map(|(_, s)| s))
        .collect()
    };
    for group in closing {
      group.complete();
    }
  }

  fn error(self, err: Err) {
    self.subs.rc_deref_mut().unsubscribe_all();
    let (groups, observer) = self.state.rc_deref_mut().take_all();
    for group in groups {
      group.error(err.clone());
    }
    if let Some(observer) = observer {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.subs.rc_deref_mut().unsubscribe_all();
    let (groups, observer) = self.state.rc_deref_mut().take_all();
    for group in groups {
      group.complete();
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

type StateRc<C, Key, CtxMarker> = <C as Context>::RcMut<
  GroupByWithState<<C as Context>::Inner, Key, <CtxMarker as Context>::Inner>,
>;
type SubsRc<C> = <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;
type GbSource<C, Key, CtxMarker, F, Conn, D> =
  GroupByWithSourceObserver<StateRc<C, Key, CtxMarker>, SubsRc<C>, F, Conn, D, CtxMarker>;
type GbDuration<'a, C, Key, CtxMarker, S> = GroupDurationObserver<
  StateRc<C, Key, CtxMarker>,
  SubsRc<C>,
  Key,
  CtxMarker,
  <S as ObservableType>::Item<'a>,
>;

impl<S, F, Key, Conn, D, CtxMarker, C, Out> CoreObservable<C>
  for GroupByWith<S, F, Conn, D, CtxMarker>
where
  C: Context,
  CtxMarker: Context<Inner: Clone + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>>,
  S: CoreObservable<C::With<GbSource<C, Key, CtxMarker, F, Conn, D>>>,
  F: for<'a> FnMut(&<S as ObservableType>::Item<'a>) -> Key,
  Key: Hash + Eq + Clone,
  Conn: Connector<Subject = CtxMarker::Inner>,
  D: FnMut(CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>) -> Out,
  Out: Context<
    Inner: for<'a> CoreObservable<
      Out::With<GbDuration<'a, C, Key, CtxMarker, S>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  >,
  C::Inner: Observer<CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>, S::Err>,
  S::Err: Clone,
{
  type Unsub = SourceWithDynamicSubs<S::Unsub, SubsRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let GroupByWith { source, key_selector, connector, duration, .. } = self;
    let state: StateRc<C, Key, CtxMarker> = C::RcMut::from(GroupByWithState {
      observer: Some(context.into_inner()),
      groups: HashMap::new(),
      dispatching: false,
      pending_close: Vec::new(),
    });
    let subs: SubsRc<C> = C::RcMut::from(DynamicSubscriptions::default());
    let observer = GroupByWithSourceObserver {
      state,
      subs: subs.clone(),
      key_selector,
      connector,
      duration,
      _marker: PhantomData,
    };
    let source_unsub = source.subscribe(C::lift(observer));
    SourceWithDynamicSubs::new(source_unsub, subs)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{ops::share::ReplayConnector, prelude::*};

  type Buckets = Rc<RefCell<Vec<(i32, Rc<RefCell<Vec<i32>>>, Rc<RefCell<bool>>)>>>;
  type Closers = Rc<RefCell<Vec<LocalSubject<'static, (), Infallible>>>>;

  fn snapshot(buckets: &Buckets) -> Vec<(i32, Vec<i32>, bool)> {
    buckets
      .borrow()
      .iter()
      .map(|(k, b, done)| (*k, b.borrow().clone(), *done.borrow()))
      .collect()
  }

  /// Collect every group's key, items and completion; durations are
  /// subjects kept in `closers`, in creation order.
  fn wire(source: &LocalSubject<'static, i32, Infallible>) -> (Buckets, Closers) {
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let closers: Closers = Rc::new(RefCell::new(Vec::new()));
    let (sink, closers_c) = (buckets.clone(), closers.clone());
    source
      .clone()
      .group_by_with_duration(
        |v: &i32| v % 2,
        move |_group| {
          let closer = Local::subject::<(), Infallible>();
          closers_c.borrow_mut().push(closer.clone());
          closer
        },
      )
      .subscribe(move |group: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        let done = Rc::new(RefCell::new(false));
        sink
          .borrow_mut()
          .push((group.inner.key, bucket.clone(), done.clone()));
        group
          .on_complete(move || *done.borrow_mut() = true)
          .subscribe(move |v| bucket.borrow_mut().push(v));
      });
    (buckets, closers)
  }

  #[rxrust_macro::test]
  fn test_group_by_with_duration_closes_and_reopens_groups() {
    let mut source = Local::subject::<i32, Infallible>();
    let (buckets, closers) = wire(&source);

    source.next(1);
    source.next(3);
    source.next(2);
    assert_eq!(snapshot(&buckets), vec![(1, vec![1, 3], false), (0, vec![2], false)]);

    // The odd group's duration fires: it completes, and 5 opens a new one.
    let mut close_odd = closers.borrow()[0].clone();
    close_odd.next(());
    source.next(5);
    assert_eq!(
      snapshot(&buckets),
      vec![(1, vec![1, 3], true), (0, vec![2], false), (1, vec![5], false)]
    );

    source.complete();
    assert_eq!(
      snapshot(&buckets),
      vec![(1, vec![1, 3], true), (0, vec![2], true), (1, vec![5], true)]
    );
    assert_eq!(closers.borrow()[1].inner.subscriber_count(), 0, "completion releases durations");
  }

  #[rxrust_macro::test]
  fn test_group_by_with_duration_from_the_group_itself() {
    // Close every group after two items, derived from the group's own
    // stream. The duration fires during the group's dispatch, so the close
    // is deferred until the item has been delivered.
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let sink = buckets.clone();

    Local::from_iter(vec![1, 3, 5, 7, 9])
      .group_by_with_duration(|_v: &i32| 0, |group: Local<_>| group.take(2).ignore_elements())
      .subscribe(move |group: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        let done = Rc::new(RefCell::new(false));
        sink
          .borrow_mut()
          .push((group.inner.key, bucket.clone(), done.clone()));
        group
          .on_complete(move || *done.borrow_mut() = true)
          .subscribe(move |v| bucket.borrow_mut().push(v));
      });

    assert_eq!(
      snapshot(&buckets),
      vec![(0, vec![1, 3], true), (0, vec![5, 7], true), (0, vec![9], true)]
    );
  }

  #[rxrust_macro::test]
  fn test_group_by_connector_replays_to_late_group_subscribers() {
    type Source = LocalSubject<'static, i32, Infallible>;
    let mut source: Source = Local::subject::<i32, Infallible>();
    let never = Local::subject::<(), Infallible>();
    let groups = Rc::new(RefCell::new(Vec::new()));
    let groups_c = groups.clone();

    source
      .clone()
      .group_by_connector(
        |v: &i32| *v > 0,
        ReplayConnector::<ReplaySubjectOf<'static, Source>>::new(None),
        move |_group| never.clone(),
      )
      .subscribe(move |group: Local<_>| groups_c.borrow_mut().push(group));

    source.next(1);
    source.next(2);
    source.next(-1);

    // Subscribing to a group later replays what it already received.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let positives = groups.borrow()[0].clone();
    positives.subscribe(move |v| sink.borrow_mut().push(v));
    assert_eq!(*seen.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_group_by_with_duration_propagates_error_to_groups() {
    let group_errors = Rc::new(RefCell::new(Vec::new()));
    let outer_errors = Rc::new(RefCell::new(Vec::new()));
    let (ge, oe) = (group_errors.clone(), outer_errors.clone());
    let mut source = Local::subject::<i32, &'static str>();
    let never = Local::subject::<(), &'static str>();

    source
      .clone()
      .group_by_with_duration(|v: &i32| *v, move |_group| never.clone())
      .on_error(move |e| oe.borrow_mut().push(e))
      .subscribe(move |group: Local<_>| {
        let ge = ge.clone();
        group
          .on_error(move |e| ge.borrow_mut().push(e))
          .subscribe(|_| {});
      });

    source.next(1);
    source.next(2);
    source.error("boom");
    assert_eq!(*group_errors.borrow(), vec!["boom", "boom"]);
    assert_eq!(*outer_errors.borrow(), vec!["boom"]);
  }
}
