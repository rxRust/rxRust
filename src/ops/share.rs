//! Share operator with RxJS 7 reset semantics.
//!
//! Multicasts a source through a subject created by a [`Connector`]. Unlike
//! `publish().ref_count()`, the subject can be reset: after an error, after
//! completion, or when the last subscriber leaves. A reset drops the subject
//! and the connection, so the next subscriber gets a fresh subject and a new
//! source subscription.

use std::marker::PhantomData;

use super::ref_count::PublishSubjectOf;
use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, Observable, ObservableType},
  observer::Observer,
  scheduler::Duration,
  subject::{MulticastSubject, ReplayBuffer, ReplaySubject, ReplaySubjectOf, Subject, Terminal},
  subscription::{IntoBoxedSubscription, Subscription},
};

/// When a shared connection is reset (RxJS `ShareConfig`).
///
/// The defaults are RxJS's: reset on error, on completion and when the
/// subscriber count drops to zero, so every "cold start" resubscribes to the
/// source. [`ShareConfig::replay`] gives RxJS `shareReplay`'s defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareConfig {
  /// Drop the subject after the source errors, so later subscribers
  /// resubscribe instead of receiving the error.
  pub reset_on_error: bool,
  /// Drop the subject after the source completes, so later subscribers
  /// resubscribe instead of receiving the completion.
  pub reset_on_complete: bool,
  /// Unsubscribe the source and drop the subject when the last subscriber
  /// leaves; otherwise the connection stays alive.
  pub reset_on_ref_count_zero: bool,
}

impl Default for ShareConfig {
  fn default() -> Self { Self::new() }
}

impl ShareConfig {
  /// RxJS `share()` defaults: reset on error, completion and ref count zero.
  pub const fn new() -> Self {
    Self { reset_on_error: true, reset_on_complete: true, reset_on_ref_count_zero: true }
  }

  /// RxJS `shareReplay()` defaults: reset on error only; the connection and
  /// the replay buffer outlive the subscribers.
  pub const fn replay() -> Self {
    Self { reset_on_error: true, reset_on_complete: false, reset_on_ref_count_zero: false }
  }

  pub const fn reset_on_error(mut self, reset: bool) -> Self {
    self.reset_on_error = reset;
    self
  }

  pub const fn reset_on_complete(mut self, reset: bool) -> Self {
    self.reset_on_complete = reset;
    self
  }

  pub const fn reset_on_ref_count_zero(mut self, reset: bool) -> Self {
    self.reset_on_ref_count_zero = reset;
    self
  }
}

/// Creates the subject for each connection of a [`Share`].
pub trait Connector {
  type Subject;
  fn create(&mut self) -> Self::Subject;
}

/// Connector for plain (publish) subjects: `Sub::default()` per connection.
pub struct PublishConnector<Sub>(PhantomData<fn() -> Sub>);

impl<Sub> Default for PublishConnector<Sub> {
  fn default() -> Self { Self(PhantomData) }
}

impl<Sub> Clone for PublishConnector<Sub> {
  fn clone(&self) -> Self { Self(PhantomData) }
}

impl<Sub: Default> Connector for PublishConnector<Sub> {
  type Subject = Sub;

  fn create(&mut self) -> Sub { Sub::default() }
}

/// Connector for replay subjects with an optional capacity and time window.
pub struct ReplayConnector<Sub> {
  pub capacity: Option<usize>,
  pub window: Option<Duration>,
  _marker: PhantomData<fn() -> Sub>,
}

impl<Sub> ReplayConnector<Sub> {
  pub fn new(capacity: Option<usize>) -> Self {
    Self { capacity, window: None, _marker: PhantomData }
  }

  pub fn with_window(mut self, window: Duration) -> Self {
    self.window = Some(window);
    self
  }
}

impl<Sub> Clone for ReplayConnector<Sub> {
  fn clone(&self) -> Self {
    Self { capacity: self.capacity, window: self.window, _marker: PhantomData }
  }
}

impl<P, B, Item, Err> Connector for ReplayConnector<ReplaySubject<P, B>>
where
  Subject<P>: Default,
  B: RcDerefMut<Target = ReplayBuffer<Item, Err>> + From<ReplayBuffer<Item, Err>>,
{
  type Subject = ReplaySubject<P, B>;

  fn create(&mut self) -> ReplaySubject<P, B> {
    match self.window {
      Some(window) => ReplaySubject::new_with_window(self.capacity, window),
      None => ReplaySubject::new(self.capacity),
    }
  }
}

/// Shared by every clone of a [`Share`] and by its subscriptions.
pub struct ShareState<Conn, Sub, BoxedSub, Err> {
  connector: Conn,
  config: ShareConfig,
  subject: Option<Sub>,
  connection: Option<BoxedSub>,
  /// The terminal event kept for late subscribers when no reset applies.
  terminal: Option<Terminal<Err>>,
  ref_count: usize,
  /// Bumped on every reset so stale observers and subscriptions stand down.
  generation: usize,
}

impl<Conn, Sub, BoxedSub, Err> ShareState<Conn, Sub, BoxedSub, Err> {
  pub fn new(connector: Conn, config: ShareConfig) -> Self {
    Self {
      connector,
      config,
      subject: None,
      connection: None,
      terminal: None,
      ref_count: 0,
      generation: 0,
    }
  }

  fn reset(&mut self) {
    self.subject = None;
    self.connection = None;
    self.terminal = None;
    self.ref_count = 0;
    self.generation += 1;
  }
}

#[doc(alias = "share")]
pub struct Share<S, StateRc> {
  pub(crate) source: S,
  pub(crate) state: StateRc,
}

impl<S: Clone, StateRc: Clone> Clone for Share<S, StateRc> {
  fn clone(&self) -> Self { Self { source: self.source.clone(), state: self.state.clone() } }
}

impl<S, StateRc> ObservableType for Share<S, StateRc>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Feeds the source into the current subject and applies the reset policy on
/// terminal events.
pub struct ShareSourceObserver<StateRc, Sub> {
  state: StateRc,
  subject: Sub,
  generation: usize,
}

impl<StateRc, Conn, Sub, BoxedSub, Item, Err> Observer<Item, Err>
  for ShareSourceObserver<StateRc, Sub>
where
  StateRc: RcDerefMut<Target = ShareState<Conn, Sub, BoxedSub, Err>>,
  Sub: Observer<Item, Err>,
  Err: Clone,
{
  fn next(&mut self, value: Item) { self.subject.next(value); }

  fn error(self, err: Err) {
    {
      let mut st = self.state.rc_deref_mut();
      if st.generation == self.generation {
        st.connection = None;
        if st.config.reset_on_error {
          st.reset();
        } else {
          st.terminal = Some(Terminal::Error(err.clone()));
        }
      }
    }
    self.subject.error(err);
  }

  fn complete(self) {
    {
      let mut st = self.state.rc_deref_mut();
      if st.generation == self.generation {
        st.connection = None;
        if st.config.reset_on_complete {
          st.reset();
        } else {
          st.terminal = Some(Terminal::Complete);
        }
      }
    }
    self.subject.complete();
  }

  /// Closed only once this connection was reset; an idle subject (no
  /// subscribers, `reset_on_ref_count_zero = false`) keeps the source alive.
  fn is_closed(&self) -> bool { self.state.rc_deref().generation != self.generation }
}

pub struct ShareSubscription<InnerSub, StateRc> {
  /// `None` when the subscriber only received a replayed terminal.
  inner: Option<InnerSub>,
  state: StateRc,
  generation: usize,
}

impl<InnerSub, StateRc, Conn, Sub, BoxedSub, Err> Subscription
  for ShareSubscription<InnerSub, StateRc>
where
  InnerSub: Subscription,
  StateRc: RcDerefMut<Target = ShareState<Conn, Sub, BoxedSub, Err>>,
  BoxedSub: Subscription,
{
  fn unsubscribe(self) {
    let Some(inner) = self.inner else { return };
    inner.unsubscribe();
    let connection = {
      let mut st = self.state.rc_deref_mut();
      if st.generation != self.generation {
        None
      } else {
        st.ref_count = st.ref_count.saturating_sub(1);
        if st.ref_count == 0 && st.config.reset_on_ref_count_zero {
          let connection = st.connection.take();
          st.reset();
          connection
        } else {
          None
        }
      }
    };
    if let Some(connection) = connection {
      connection.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .inner
      .as_ref()
      .is_none_or(|inner| inner.is_closed())
  }
}

/// The state pointer a context uses for a [`Share`] with connector `Conn`.
pub type ShareStateOf<O, Conn> = <O as Context>::RcMut<
  ShareState<
    Conn,
    <Conn as Connector>::Subject,
    <O as Context>::BoxedSubscription,
    <O as Observable>::Err,
  >,
>;

/// `share_with` result: a [`Share`] over a publish subject.
pub type ShareWithOf<'a, O> = <O as Context>::With<
  Share<<O as Context>::Inner, ShareStateOf<O, PublishConnector<PublishSubjectOf<'a, O>>>>,
>;

/// `share_replay_with` result: a [`Share`] over a replay subject.
pub type ShareReplayWithOf<'a, O> = <O as Context>::With<
  Share<<O as Context>::Inner, ShareStateOf<O, ReplayConnector<ReplaySubjectOf<'a, O>>>>,
>;

impl<S, C, StateRc, Conn, Sub> CoreObservable<C> for Share<S, StateRc>
where
  C: Context + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>,
  StateRc: Clone + RcDerefMut<Target = ShareState<Conn, Sub, C::BoxedSubscription, S::Err>>,
  Conn: Connector<Subject = Sub>,
  Sub: MulticastSubject + Clone + CoreObservable<C>,
  S: Clone
    + CoreObservable<
      C::With<ShareSourceObserver<StateRc, Sub>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  S::Err: Clone,
  C::BoxedSubscription: Subscription,
{
  type Unsub = ShareSubscription<<Sub as CoreObservable<C>>::Unsub, StateRc>;

  fn subscribe(self, observer: C) -> Self::Unsub {
    let Share { source, state } = self;

    // Ended without a reset: late subscribers get the terminal, as they
    // would from an RxJS subject. A replay subject delivers its buffer and
    // the terminal itself; a plain subject has no terminal record.
    let terminal = {
      let st = state.rc_deref();
      match (&st.terminal, &st.subject) {
        (Some(terminal), Some(subject)) if !subject.is_terminated() => Some(terminal.clone()),
        (Some(terminal), None) => Some(terminal.clone()),
        _ => None,
      }
    };
    if let Some(terminal) = terminal {
      match terminal {
        Terminal::Error(err) => observer.error(err),
        Terminal::Complete => observer.complete(),
      }
      return ShareSubscription { inner: None, state, generation: 0 };
    }

    let (subject, generation) = {
      let mut st = state.rc_deref_mut();
      if st.subject.is_none() {
        let subject = st.connector.create();
        st.subject = Some(subject);
      }
      st.ref_count += 1;
      (
        st.subject
          .clone()
          .expect("subject was just created"),
        st.generation,
      )
    };

    let inner = subject.clone().subscribe(observer);

    // Connect on the first subscriber of a generation.
    let needs_connect = {
      let st = state.rc_deref();
      st.generation == generation && st.connection.is_none() && st.terminal.is_none()
    };
    if needs_connect {
      let source_observer =
        ShareSourceObserver { state: state.clone(), subject: subject.clone(), generation };
      let connection = source
        .subscribe(C::lift(source_observer))
        .into_boxed();
      // A source that ended synchronously already reset or terminated the
      // state; its handle has nothing left to cancel.
      let mut st = state.rc_deref_mut();
      if st.generation == generation && st.terminal.is_none() {
        st.connection = Some(connection);
      }
    }

    ShareSubscription { inner: Some(inner), state, generation }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use super::*;
  use crate::prelude::*;

  type Log = Rc<RefCell<Vec<i32>>>;

  fn sink(log: &Log) -> impl FnMut(i32) + 'static {
    let log = log.clone();
    move |v| log.borrow_mut().push(v)
  }

  #[rxrust_macro::test]
  fn test_share_with_connects_once_for_two_subscribers() {
    let mut source = Local::subject::<i32, Infallible>();
    let shared = source.clone().share_with(ShareConfig::default());
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    let sub_a = shared.clone().subscribe(sink(&a));
    let sub_b = shared.clone().subscribe(sink(&b));
    assert_eq!(source.inner.subscriber_count(), 1, "one source connection");

    source.next(1);
    assert_eq!(*a.borrow(), vec![1]);
    assert_eq!(*b.borrow(), vec![1]);

    // The connection survives until the last subscriber leaves.
    sub_a.unsubscribe();
    source.next(2);
    assert_eq!(*b.borrow(), vec![1, 2]);
    sub_b.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 0, "reset on ref count zero disconnects");

    // A new subscriber reconnects.
    let _sub_c = shared.subscribe(sink(&a));
    assert_eq!(source.inner.subscriber_count(), 1);
  }

  #[rxrust_macro::test]
  fn test_share_with_keeps_connection_without_ref_count_reset() {
    let mut source = Local::subject::<i32, Infallible>();
    let shared = source
      .clone()
      .share_replay_with(Some(1), ShareConfig::default().reset_on_ref_count_zero(false));
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    let sub_a = shared.clone().subscribe(sink(&a));
    source.next(1);
    sub_a.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 1, "connection stays alive");
    source.next(2); // buffered by the live replay subject

    shared.subscribe(sink(&b));
    assert_eq!(*b.borrow(), vec![2], "late subscriber joins the same subject");
    assert_eq!(source.inner.subscriber_count(), 1);
  }

  #[rxrust_macro::test]
  fn test_share_replay_with_reset_on_ref_count_zero_drops_buffer() {
    let mut source = Local::subject::<i32, Infallible>();
    let shared = source
      .clone()
      .share_replay_with(Some(1), ShareConfig::default());
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    let sub_a = shared.clone().subscribe(sink(&a));
    source.next(1);
    sub_a.unsubscribe();

    shared.subscribe(sink(&b));
    assert!(b.borrow().is_empty(), "a fresh subject has no buffer");
  }

  #[rxrust_macro::test]
  fn test_share_with_resubscribes_after_completion() {
    let shared = Local::from_iter(vec![1, 2]).share_with(ShareConfig::default());
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    shared.clone().subscribe(sink(&a));
    shared.subscribe(sink(&b));
    assert_eq!(*a.borrow(), vec![1, 2]);
    assert_eq!(*b.borrow(), vec![1, 2], "reset on complete: the source runs again");
  }

  #[rxrust_macro::test]
  fn test_share_with_no_reset_on_complete_replays_completion() {
    let shared =
      Local::from_iter(vec![1, 2]).share_with(ShareConfig::default().reset_on_complete(false));
    let (a, b): (Log, Log) = (Default::default(), Default::default());
    let done = Rc::new(RefCell::new(false));
    let done_c = done.clone();

    shared.clone().subscribe(sink(&a));
    shared
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(sink(&b));
    assert_eq!(*a.borrow(), vec![1, 2]);
    assert!(b.borrow().is_empty(), "late subscriber only sees the completion");
    assert!(*done.borrow());
  }

  #[rxrust_macro::test]
  fn test_share_with_reset_on_error() {
    let attempts = Rc::new(RefCell::new(0));
    let attempts_c = attempts.clone();
    let shared = Local::throw_err::<&'static str>("boom")
      .tap(move |_: &()| ())
      .finalize(move || *attempts_c.borrow_mut() += 1)
      .share_with(ShareConfig::default());
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (e1, e2) = (errors.clone(), errors.clone());

    shared
      .clone()
      .on_error(move |e| e1.borrow_mut().push(e))
      .subscribe(|_: ()| {});
    shared
      .on_error(move |e| e2.borrow_mut().push(e))
      .subscribe(|_: ()| {});

    assert_eq!(*errors.borrow(), vec!["boom", "boom"]);
    assert_eq!(*attempts.borrow(), 2, "reset on error: the source runs again");
  }

  #[rxrust_macro::test]
  fn test_share_with_no_reset_on_error_replays_error() {
    let attempts = Rc::new(RefCell::new(0));
    let attempts_c = attempts.clone();
    let shared = Local::throw_err::<&'static str>("boom")
      .tap(move |_: &()| ())
      .finalize(move || *attempts_c.borrow_mut() += 1)
      .share_with(ShareConfig::default().reset_on_error(false));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (e1, e2) = (errors.clone(), errors.clone());

    shared
      .clone()
      .on_error(move |e| e1.borrow_mut().push(e))
      .subscribe(|_: ()| {});
    shared
      .on_error(move |e| e2.borrow_mut().push(e))
      .subscribe(|_: ()| {});

    assert_eq!(*errors.borrow(), vec!["boom", "boom"]);
    assert_eq!(*attempts.borrow(), 1, "the terminated subject replays the error");
  }

  #[rxrust_macro::test]
  fn test_share_replay_config_matches_rxjs_share_replay() {
    let shared = Local::from_iter(vec![1, 2]).share_replay_with(Some(2), ShareConfig::replay());
    let (a, b): (Log, Log) = (Default::default(), Default::default());
    let done = Rc::new(RefCell::new(false));
    let done_c = done.clone();

    shared.clone().subscribe(sink(&a));
    shared
      .on_complete(move || *done_c.borrow_mut() = true)
      .subscribe(sink(&b));
    assert_eq!(*a.borrow(), vec![1, 2]);
    assert_eq!(*b.borrow(), vec![1, 2], "buffer and completion replayed, no resubscribe");
    assert!(*done.borrow());
  }

  #[rxrust_macro::test]
  fn test_share_connector_accepts_a_custom_connector() {
    type Source = LocalSubject<'static, i32, Infallible>;
    let mut source: Source = Local::subject::<i32, Infallible>();
    let shared = source.clone().share_connector(
      ReplayConnector::<ReplaySubjectOf<'static, Source>>::new(None),
      ShareConfig::replay(),
    );
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    shared.clone().subscribe(sink(&a));
    source.next(1);
    shared.subscribe(sink(&b));
    assert_eq!(*b.borrow(), vec![1]);
  }

  #[rxrust_macro::test]
  fn test_share_with_works_in_shared_context() {
    use std::sync::{Arc, Mutex};

    let mut source = Shared::subject::<i32, Infallible>();
    let shared = source.clone().share_with(ShareConfig::default());
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_c = log.clone();

    let sub = shared.subscribe(move |v| log_c.lock().unwrap().push(v));
    source.next(7);
    sub.unsubscribe();
    assert_eq!(*log.lock().unwrap(), vec![7]);
    assert_eq!(source.inner.subscriber_count(), 0);
  }
}
