//! Window operator implementation
//!
//! Splits the source into consecutive windows, each an observable of its own,
//! delimited by a notifier.

use std::{convert::Infallible, marker::PhantomData};

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, Interval, ObservableType},
  observer::Observer,
  ops::{map_err::MapErr, ref_count::PublishSubjectOf},
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// Maps an impossible error into any error type; used to feed infallible
/// notifiers (timers) into operators that expect the source's error type.
pub fn never_errors<E>(never: Infallible) -> E { match never {} }

/// Window operator: Emit consecutive windows delimited by `notifier`
///
/// Each window is a `Subject` wrapped in the context, emitted as soon as it
/// opens; the first window opens at subscribe. When `notifier` emits, the
/// current window completes and a new one opens. Source completion or error
/// terminates the open window and then the outer stream. Items must be
/// `Clone`.
///
/// # Examples
///
/// ```rust
/// use std::{cell::RefCell, convert::Infallible, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let windows = Rc::new(RefCell::new(Vec::new()));
/// let mut source = Local::subject::<i32, Infallible>();
/// let mut boundary = Local::subject::<(), Infallible>();
///
/// let sink = windows.clone();
/// source
///   .clone()
///   .window(boundary.clone())
///   .subscribe(move |w: Local<_>| {
///     let bucket = Rc::new(RefCell::new(Vec::new()));
///     sink.borrow_mut().push(bucket.clone());
///     w.subscribe(move |v| bucket.borrow_mut().push(v));
///   });
///
/// source.next(1);
/// source.next(2);
/// boundary.next(());
/// source.next(3);
/// source.complete();
///
/// let seen: Vec<Vec<i32>> = windows
///   .borrow()
///   .iter()
///   .map(|b| b.borrow().clone())
///   .collect();
/// assert_eq!(seen, vec![vec![1, 2], vec![3]]);
/// ```
pub struct Window<S, N, CtxMarker> {
  pub source: S,
  pub notifier: N,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<S: Clone, N: Clone, CtxMarker> Clone for Window<S, N, CtxMarker> {
  fn clone(&self) -> Self {
    Self { source: self.source.clone(), notifier: self.notifier.clone(), _marker: PhantomData }
  }
}

impl<S, N, CtxMarker> Window<S, N, CtxMarker> {
  /// Pairs a source with its boundary notifier.
  pub fn new(source: S, notifier: N) -> Self { Self { source, notifier, _marker: PhantomData } }
}

impl<S, N, CtxMarker> ObservableType for Window<S, N, CtxMarker>
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

/// State shared by the source and notifier observers
pub struct WindowState<O, Sub> {
  observer: Option<O>,
  current: Option<Sub>,
}

/// Completes the open window, if any, and opens the next one.
pub(crate) fn rotate_window<CtxMarker, O, Item, Err>(state: &mut WindowState<O, CtxMarker::Inner>)
where
  CtxMarker: Context<Inner: Default + Clone + Observer<Item, Err>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
{
  if let Some(window) = state.current.take() {
    window.complete();
  }
  let Some(observer) = state.observer.as_mut() else { return };
  if observer.is_closed() {
    return;
  }
  let subject = CtxMarker::Inner::default();
  observer.next(CtxMarker::lift(subject.clone()));
  state.current = Some(subject);
}

/// Observer for the source
pub struct WindowSourceObserver<R, NotifierUnsub, CtxMarker> {
  state: R,
  notifier_unsub: NotifierUnsub,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<R, NotifierUnsub, CtxMarker, O, Item, Err> Observer<Item, Err>
  for WindowSourceObserver<R, NotifierUnsub, CtxMarker>
where
  CtxMarker: Context<Inner: Observer<Item, Err> + Clone>,
  R: RcDerefMut<Target = WindowState<O, CtxMarker::Inner>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  NotifierUnsub: Subscription,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    if let Some(window) = self.state.rc_deref_mut().current.as_mut() {
      window.next(value);
    }
  }

  fn error(self, err: Err) {
    self.notifier_unsub.unsubscribe();
    let mut state = self.state.rc_deref_mut();
    if let Some(window) = state.current.take() {
      window.error(err.clone());
    }
    if let Some(observer) = state.observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.notifier_unsub.unsubscribe();
    let mut state = self.state.rc_deref_mut();
    if let Some(window) = state.current.take() {
      window.complete();
    }
    if let Some(observer) = state.observer.take() {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref_mut()
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

/// Observer for the boundary notifier
pub struct WindowNotifierObserver<R, SourceUnsub, CtxMarker, Item> {
  state: R,
  source_unsub: SourceUnsub,
  _marker: PhantomData<fn() -> (CtxMarker, Item)>,
}

impl<R, SourceUnsub, CtxMarker, Item, O, NotifyItem, Err> Observer<NotifyItem, Err>
  for WindowNotifierObserver<R, SourceUnsub, CtxMarker, Item>
where
  CtxMarker: Context<Inner: Default + Clone + Observer<Item, Err>>,
  R: RcDerefMut<Target = WindowState<O, CtxMarker::Inner>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  SourceUnsub: Subscription,
  Err: Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    rotate_window::<CtxMarker, O, Item, Err>(&mut self.state.rc_deref_mut());
  }

  fn error(self, err: Err) {
    self.source_unsub.unsubscribe();
    let mut state = self.state.rc_deref_mut();
    if let Some(window) = state.current.take() {
      window.error(err.clone());
    }
    if let Some(observer) = state.observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    // A completed boundary keeps the current window open until the source
    // ends, as in RxJS.
  }

  fn is_closed(&self) -> bool { self.state.rc_deref_mut().observer.is_none() }
}

type WState<C, CtxMarker> =
  <C as Context>::RcMut<WindowState<<C as Context>::Inner, <CtxMarker as Context>::Inner>>;
type WSourceObserver<C, CtxMarker> = WindowSourceObserver<
  WState<C, CtxMarker>,
  <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>,
  CtxMarker,
>;
type WNotifierObserver<'a, C, CtxMarker, SourceUnsub, S> = WindowNotifierObserver<
  WState<C, CtxMarker>,
  <C as Context>::RcMut<Option<SourceUnsub>>,
  CtxMarker,
  <S as ObservableType>::Item<'a>,
>;

impl<S, N, CtxMarker, C, SourceUnsub, NotifierUnsub> CoreObservable<C> for Window<S, N, CtxMarker>
where
  C: Context,
  S: CoreObservable<C::With<WSourceObserver<C, CtxMarker>>, Unsub = SourceUnsub>,
  CtxMarker:
    Context<Inner: Default + Clone + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>>,
  C::Inner: Observer<CtxMarker::With<CtxMarker::Inner>, S::Err>,
  N: for<'a> CoreObservable<
      C::With<WNotifierObserver<'a, C, CtxMarker, SourceUnsub, S>>,
      Unsub = NotifierUnsub,
    >,
  SourceUnsub: Subscription,
  NotifierUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  C::RcMut<Option<SourceUnsub>>: Subscription,
  C::RcMut<Option<C::BoxedSubscription>>: Subscription,
{
  type Unsub =
    TupleSubscription<C::RcMut<Option<SourceUnsub>>, C::RcMut<Option<C::BoxedSubscription>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Window { source, notifier, .. } = self;

    let state: WState<C, CtxMarker> =
      C::RcMut::from(WindowState { observer: Some(context.into_inner()), current: None });
    rotate_window::<CtxMarker, C::Inner, S::Item<'_>, S::Err>(&mut state.rc_deref_mut());

    let source_unsub_proxy: C::RcMut<Option<SourceUnsub>> = C::RcMut::from(None);
    let notifier_unsub_proxy: C::RcMut<Option<C::BoxedSubscription>> = C::RcMut::from(None);

    let source_observer = WindowSourceObserver {
      state: state.clone(),
      notifier_unsub: notifier_unsub_proxy.clone(),
      _marker: PhantomData,
    };
    let source_sub = source.subscribe(C::lift(source_observer));
    *source_unsub_proxy.rc_deref_mut() = Some(source_sub);

    let notifier_observer = WindowNotifierObserver {
      state: state.clone(),
      source_unsub: source_unsub_proxy.clone(),
      _marker: PhantomData,
    };
    let notifier_sub = notifier.subscribe(C::lift(notifier_observer));
    *notifier_unsub_proxy.rc_deref_mut() = Some(notifier_sub.into_boxed());

    TupleSubscription::new(source_unsub_proxy, notifier_unsub_proxy)
  }
}

/// The context-wrapped `Subject` that `window`, `window_count` and
/// `window_time` emit for an observable `O`.
pub type WindowSubjectOf<'a, O> = <O as Context>::With<PublishSubjectOf<'a, O>>;

/// The timer notifier `window_time` builds: an `Interval` whose impossible
/// error is mapped into the source's error type.
pub type WindowTimer<Sch, Err> = MapErr<Interval<Sch>, fn(Infallible) -> Err>;

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  type Buckets = Rc<RefCell<Vec<Rc<RefCell<Vec<i32>>>>>>;

  fn snapshot(buckets: &Buckets) -> Vec<Vec<i32>> {
    buckets
      .borrow()
      .iter()
      .map(|b| b.borrow().clone())
      .collect()
  }

  #[rxrust_macro::test]
  fn test_window_rotates_on_notifier() {
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let sink = buckets.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let mut boundary = Local::subject::<(), Infallible>();

    source
      .clone()
      .window(boundary.clone())
      .subscribe(move |w: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        sink.borrow_mut().push(bucket.clone());
        w.subscribe(move |v| bucket.borrow_mut().push(v));
      });

    // The first window opens at subscribe
    assert_eq!(snapshot(&buckets), vec![Vec::<i32>::new()]);
    source.next(1);
    source.next(2);
    boundary.next(());
    source.next(3);
    boundary.next(());
    source.complete();

    assert_eq!(snapshot(&buckets), vec![vec![1, 2], vec![3], vec![]]);
  }

  #[rxrust_macro::test]
  fn test_window_completes_open_window_on_source_completion() {
    let completed = Rc::new(RefCell::new(0));
    let outer_done = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();
    let outer_c = outer_done.clone();
    let mut source = Local::subject::<i32, Infallible>();
    let boundary = Local::subject::<(), Infallible>();

    source
      .clone()
      .window(boundary.clone())
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
    assert_eq!(boundary.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_window_count_groups() {
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let sink = buckets.clone();

    Local::from_iter(vec![1, 2, 3, 4, 5])
      .window_count(2)
      .subscribe(move |w: Local<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        sink.borrow_mut().push(bucket.clone());
        w.subscribe(move |v| bucket.borrow_mut().push(v));
      });

    assert_eq!(snapshot(&buckets), vec![vec![1, 2], vec![3, 4], vec![5]]);
  }

  #[rxrust_macro::test]
  fn test_window_time_rotates_on_virtual_clock() {
    TestScheduler::init();
    let buckets: Buckets = Rc::new(RefCell::new(Vec::new()));
    let sink = buckets.clone();
    let mut source = TestCtx::subject::<i32, Infallible>();

    let _sub = source
      .clone()
      .window_time(Duration::from_millis(100))
      .subscribe(move |w: TestCtx<_>| {
        let bucket = Rc::new(RefCell::new(Vec::new()));
        sink.borrow_mut().push(bucket.clone());
        w.subscribe(move |v| bucket.borrow_mut().push(v));
      });

    source.next(1);
    TestScheduler::advance_by(Duration::from_millis(100));
    source.next(2);
    source.next(3);
    TestScheduler::advance_by(Duration::from_millis(100));
    source.clone().complete();

    assert_eq!(snapshot(&buckets), vec![vec![1], vec![2, 3], vec![]]);
  }
}
