//! RefCount operator - auto-manages ConnectableObservable connection.
//!
//! Connects on first subscription, disconnects when all subscribers leave.
//!
//! # Example
//!
//! ```rust
//! use rxrust::prelude::*;
//!
//! let shared = Local::from_iter([1, 2]).publish().ref_count();
//!
//! let sub = shared.subscribe(|v| println!("Got: {}", v));
//! sub.unsubscribe();
//! ```
//!
//! # Grace period
//!
//! [`RefCountGrace`] is the same operator with a delay before the source is
//! disconnected, so a subscriber that arrives shortly after the last one left
//! rejoins the live connection instead of restarting the source. See
//! [`Connectable::ref_count_grace`](crate::observable::connectable::Connectable::ref_count_grace).

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType, connectable::ConnectableObservable},
  scheduler::{Duration, Schedulable, Scheduler, Task, TaskHandle, TaskState},
  subject::{Subject, subscribers::Subscribers},
  subscription::Subscription,
};

/// Wraps a `ConnectableObservable` and manages connection based on subscriber
/// count.
///
/// Uses the Subject's internal subscriber list instead of a separate counter.
pub struct RefCount<S, P, ConnPtr> {
  pub(crate) connectable: ConnectableObservable<S, P>,
  pub(crate) connection: ConnPtr,
}

impl<S: Clone, P: Clone, ConnPtr: Clone> Clone for RefCount<S, P, ConnPtr> {
  fn clone(&self) -> Self {
    Self { connectable: self.connectable.clone(), connection: self.connection.clone() }
  }
}

impl<S, P, ConnPtr> ObservableType for RefCount<S, P, ConnPtr>
where
  Subject<P>: ObservableType,
{
  type Item<'a>
    = <Subject<P> as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <Subject<P> as ObservableType>::Err;
}

impl<Ctx, O, S, P, ConnPtr> CoreObservable<Ctx> for RefCount<S, P, ConnPtr>
where
  Ctx: Context,
  S: Clone + CoreObservable<Ctx::With<Subject<P>>>,
  Subject<P>: CoreObservable<Ctx>,
  P: Clone + RcDeref<Target = Subscribers<O>>,
  ConnPtr: Clone + RcDerefMut<Target = Option<S::Unsub>> + Subscription,
{
  type Unsub = RefCountSubscription<P, <Subject<P> as CoreObservable<Ctx>>::Unsub, ConnPtr>;

  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let subject = self.connectable.fork();
    let inner_sub = subject.clone().subscribe(observer);

    if subject.subscriber_count() == 1 && self.connection.rc_deref().is_none() {
      *self.connection.rc_deref_mut() = Some(self.connectable.connect::<Ctx>());
    }

    RefCountSubscription { subject, inner: inner_sub, connection: self.connection }
  }
}

/// Subscription for RefCount. Disconnects source when last subscriber leaves.
pub struct RefCountSubscription<P, InnerSub, ConnPtr> {
  subject: Subject<P>,
  inner: InnerSub,
  connection: ConnPtr,
}

impl<P, InnerSub, ConnPtr, O> Subscription for RefCountSubscription<P, InnerSub, ConnPtr>
where
  P: RcDeref<Target = Subscribers<O>>,
  InnerSub: Subscription,
  ConnPtr: Subscription,
{
  fn unsubscribe(self) {
    self.inner.unsubscribe();
    if self.subject.is_empty() {
      self.connection.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

// ==================== RefCount with a grace period ====================

/// Connection bookkeeping shared by every clone of a [`RefCountGrace`] and by
/// all of its subscriptions.
///
/// `epoch` is bumped under the same lock that guards `connection` and
/// `pending` on *every* transition (a subscriber arriving, a reset being
/// scheduled, a reset firing). A scheduled reset carries the epoch it was
/// created with and stands down when the two no longer match, so a reset can
/// never tear down a connection that has since gained a subscriber.
pub struct GraceState<Unsub> {
  /// The live source subscription, while connected.
  connection: Option<Unsub>,
  /// The in-flight reset timer, while the grace window is open.
  pending: Option<TaskHandle>,
  epoch: usize,
}

impl<Unsub> GraceState<Unsub> {
  pub(crate) fn new() -> Self { Self { connection: None, pending: None, epoch: 0 } }
}

impl<Unsub> Default for GraceState<Unsub> {
  fn default() -> Self { Self::new() }
}

/// The task state a pending reset carries: the subject to re-check, the shared
/// state, and the epoch the reset was scheduled with.
pub type GraceTask<P, StatePtr> = (Subject<P>, StatePtr, usize);

/// Wraps a `ConnectableObservable` and manages the connection by subscriber
/// count, with a grace period before disconnecting.
///
/// Like [`RefCount`], but when the last subscriber leaves the source is kept
/// alive for `grace`. A subscriber arriving inside that window cancels the
/// teardown and joins the existing connection; if none does, the source is
/// unsubscribed when the window expires.
///
/// Equivalent to RxJava's `refCount(timeout, unit)` on
/// `ConnectableObservable`, and to RxJS 7's
/// `share({ resetOnRefCountZero: () => timer(grace) })`.
pub struct RefCountGrace<S, P, StatePtr, Sch> {
  pub(crate) connectable: ConnectableObservable<S, P>,
  pub(crate) state: StatePtr,
  pub(crate) grace: Duration,
  pub(crate) scheduler: Sch,
}

impl<S: Clone, P: Clone, StatePtr: Clone, Sch: Clone> Clone for RefCountGrace<S, P, StatePtr, Sch> {
  fn clone(&self) -> Self {
    Self {
      connectable: self.connectable.clone(),
      state: self.state.clone(),
      grace: self.grace,
      scheduler: self.scheduler.clone(),
    }
  }
}

impl<S, P, StatePtr, Sch> ObservableType for RefCountGrace<S, P, StatePtr, Sch>
where
  Subject<P>: ObservableType,
{
  type Item<'a>
    = <Subject<P> as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <Subject<P> as ObservableType>::Err;
}

/// Static handler for an expired grace window.
///
/// Runs on the scheduler after `grace` has elapsed. It disconnects the source
/// only if its epoch is still current *and* the subject is still empty; either
/// check failing means a subscriber won the race and the reset is abandoned.
fn grace_reset_handler<P, StatePtr, Unsub, O>(task_state: &mut GraceTask<P, StatePtr>) -> TaskState
where
  P: RcDeref<Target = Subscribers<O>>,
  StatePtr: RcDerefMut<Target = GraceState<Unsub>>,
  Unsub: Subscription,
{
  let (subject, state, epoch) = task_state;

  // Decide under the lock, act outside it: a teardown may call back into user
  // code, which must not re-enter this state while it is borrowed.
  let connection = {
    let mut st = state.rc_deref_mut();
    if st.epoch != *epoch {
      // Superseded - a subscriber arrived, or another reset already ran.
      None
    } else if !subject.is_empty() {
      st.pending = None;
      None
    } else {
      st.pending = None;
      st.epoch = st.epoch.wrapping_add(1);
      st.connection.take()
    }
  };

  connection.unsubscribe();

  TaskState::Finished
}

impl<Ctx, O, S, P, StatePtr, Sch> CoreObservable<Ctx> for RefCountGrace<S, P, StatePtr, Sch>
where
  Ctx: Context,
  S: CoreObservable<Ctx::With<Subject<P>>>,
  Subject<P>: CoreObservable<Ctx>,
  P: Clone + RcDeref<Target = Subscribers<O>>,
  StatePtr: Clone + RcDerefMut<Target = GraceState<S::Unsub>>,
  S::Unsub: Subscription,
  Sch: Scheduler<Task<GraceTask<P, StatePtr>>>,
  Task<GraceTask<P, StatePtr>>: Schedulable<Sch>,
{
  type Unsub =
    RefCountGraceSubscription<P, <Subject<P> as CoreObservable<Ctx>>::Unsub, StatePtr, Sch>;

  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let RefCountGrace { connectable, state, grace, scheduler } = self;

    // A new subscriber closes any open grace window.
    let (to_cancel, epoch, needs_connect) = {
      let mut st = state.rc_deref_mut();
      st.epoch = st.epoch.wrapping_add(1);
      (st.pending.take(), st.epoch, st.connection.is_none())
    };
    to_cancel.unsubscribe();

    let subject = connectable.fork();
    let inner = subject.clone().subscribe(observer);

    if needs_connect {
      let connection = connectable.connect::<Ctx>();
      // `connect` may run a synchronous source to completion, so a bumped
      // epoch means nobody is tracking this handle - release it, don't park it.
      let redundant = {
        let mut st = state.rc_deref_mut();
        if st.epoch == epoch && st.connection.is_none() {
          st.connection = Some(connection);
          None
        } else {
          Some(connection)
        }
      };
      redundant.unsubscribe();
    }

    RefCountGraceSubscription { subject, inner, state, grace, scheduler }
  }
}

/// Subscription for [`RefCountGrace`]. Starts the grace window when the last
/// subscriber leaves.
pub struct RefCountGraceSubscription<P, InnerSub, StatePtr, Sch> {
  subject: Subject<P>,
  inner: InnerSub,
  state: StatePtr,
  grace: Duration,
  scheduler: Sch,
}

impl<P, InnerSub, StatePtr, Sch, Unsub, O> Subscription
  for RefCountGraceSubscription<P, InnerSub, StatePtr, Sch>
where
  P: Clone + RcDeref<Target = Subscribers<O>>,
  InnerSub: Subscription,
  StatePtr: Clone + RcDerefMut<Target = GraceState<Unsub>>,
  Unsub: Subscription,
  Sch: Scheduler<Task<GraceTask<P, StatePtr>>>,
  Task<GraceTask<P, StatePtr>>: Schedulable<Sch>,
{
  fn unsubscribe(self) {
    let Self { subject, inner, state, grace, scheduler } = self;
    inner.unsubscribe();

    if !subject.is_empty() {
      // Other subscribers remain; the connection is still wanted.
      return;
    }

    if grace.is_zero() {
      // No window to open - behave exactly like `RefCount`.
      let (pending, connection) = {
        let mut st = state.rc_deref_mut();
        st.epoch = st.epoch.wrapping_add(1);
        (st.pending.take(), st.connection.take())
      };
      pending.unsubscribe();
      connection.unsubscribe();
      return;
    }

    let epoch = {
      let mut st = state.rc_deref_mut();
      st.epoch = st.epoch.wrapping_add(1);
      st.epoch
    };

    let task =
      Task::new((subject, state.clone(), epoch), grace_reset_handler::<P, StatePtr, Unsub, O>);
    let handle = scheduler.schedule(task, Some(grace));

    // Ran inline, or someone subscribed while we scheduled: handle is stale.
    let stale = {
      let mut st = state.rc_deref_mut();
      if st.epoch == epoch {
        st.pending = Some(handle);
        None
      } else {
        Some(handle)
      }
    };
    stale.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::{observable::Observable, prelude::*};

  #[rxrust_macro::test]
  fn test_ref_count_basic() {
    let results = Rc::new(RefCell::new(vec![]));
    let r = results.clone();

    let mut source = Local::subject();
    let shared = source.clone().publish().ref_count();

    shared.subscribe(move |v| r.borrow_mut().push(v));
    source.next(42);

    assert_eq!(*results.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_ref_count_multiple_subscribers() {
    let results1 = Rc::new(RefCell::new(vec![]));
    let results2 = Rc::new(RefCell::new(vec![]));

    let mut subject = Local::subject();
    let shared = subject.clone().publish().ref_count();

    let r1 = results1.clone();
    let _sub1 = shared
      .clone()
      .subscribe(move |v| r1.borrow_mut().push(v));

    let r2 = results2.clone();
    let _sub2 = shared.subscribe(move |v| r2.borrow_mut().push(v));

    subject.next(1);
    subject.next(2);

    assert_eq!(*results1.borrow(), vec![1, 2]);
    assert_eq!(*results2.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_ref_count_unsubscribe() {
    let results = Rc::new(RefCell::new(vec![]));

    let mut subject = Local::subject();
    let shared = subject.clone().publish().ref_count();

    let r1 = results.clone();
    let sub1 = shared
      .clone()
      .subscribe(move |v| r1.borrow_mut().push(format!("A:{v}")));

    subject.next(1);

    let r2 = results.clone();
    let sub2 = shared.subscribe(move |v| r2.borrow_mut().push(format!("B:{v}")));

    subject.next(2);
    sub1.unsubscribe();
    subject.next(3);
    sub2.unsubscribe();

    let received = results.borrow();
    assert!(received.contains(&"A:1".to_string()));
    assert!(received.contains(&"A:2".to_string()));
    assert!(received.contains(&"B:2".to_string()));
    assert!(received.contains(&"B:3".to_string()));
    assert!(!received.contains(&"A:3".to_string()));
  }
}

#[cfg(test)]
mod grace_tests {
  use std::{
    cell::RefCell,
    convert::Infallible,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  use crate::{observable::Observable, prelude::*};

  const GRACE: Duration = Duration::from_millis(50);

  type Log = Rc<RefCell<Vec<i32>>>;

  fn sink(log: &Log) -> impl FnMut(i32) + 'static {
    let log = log.clone();
    move |v| log.borrow_mut().push(v)
  }

  #[rxrust_macro::test]
  fn grace_holds_the_connection_open() {
    TestScheduler::init();

    let source = TestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);

    let sub = shared.subscribe(|_: i32| {});
    assert_eq!(source.inner.subscriber_count(), 1, "connected on first subscriber");

    sub.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 1, "still connected during the grace window");

    TestScheduler::advance_by(Duration::from_millis(49));
    assert_eq!(source.inner.subscriber_count(), 1, "the window has not expired yet");
  }

  #[rxrust_macro::test]
  fn grace_expiry_disconnects_the_source() {
    TestScheduler::init();

    let source = TestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);

    shared.subscribe(|_: i32| {}).unsubscribe();
    TestScheduler::advance_by(GRACE);

    assert_eq!(source.inner.subscriber_count(), 0, "the window expired, the source is released");
  }

  #[rxrust_macro::test]
  fn resubscribing_within_the_grace_window_cancels_the_reset() {
    TestScheduler::init();

    let mut source = TestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    let sub_a = shared.clone().subscribe(sink(&a));
    source.next(1);
    sub_a.unsubscribe();

    // Halfway through the window a new subscriber arrives.
    TestScheduler::advance_by(Duration::from_millis(25));
    let _sub_b = shared.subscribe(sink(&b));

    // Past the point where the original reset would have fired.
    TestScheduler::advance_by(Duration::from_millis(50));

    assert_eq!(
      source.inner.subscriber_count(),
      1,
      "the pending reset was cancelled, not merely delayed"
    );

    source.next(2);
    assert_eq!(*a.borrow(), vec![1]);
    assert_eq!(*b.borrow(), vec![2]);
  }

  #[rxrust_macro::test]
  fn the_grace_window_restarts_after_a_resubscribe() {
    TestScheduler::init();

    let source = TestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);

    shared
      .clone()
      .subscribe(|_: i32| {})
      .unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(25));

    let sub_b = shared.subscribe(|_: i32| {});
    sub_b.unsubscribe();

    // 25ms + 40ms is past the first window's deadline but not the second's.
    TestScheduler::advance_by(Duration::from_millis(40));
    assert_eq!(source.inner.subscriber_count(), 1, "a full fresh window, not the remainder");

    TestScheduler::advance_by(Duration::from_millis(15));
    assert_eq!(source.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn zero_grace_behaves_like_ref_count() {
    TestScheduler::init();

    let source = TestCtx::subject::<i32, Infallible>();
    let shared = source
      .clone()
      .publish()
      .ref_count_grace(Duration::ZERO);

    shared.subscribe(|_: i32| {}).unsubscribe();

    assert_eq!(source.inner.subscriber_count(), 0, "disconnected synchronously");
    assert_eq!(TestScheduler::pending_count(), 0, "no task is scheduled for a zero window");
  }

  #[rxrust_macro::test]
  fn only_the_last_subscriber_opens_the_window() {
    TestScheduler::init();

    let source = TestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);

    let sub_a = shared.clone().subscribe(|_: i32| {});
    let sub_b = shared.subscribe(|_: i32| {});
    assert_eq!(source.inner.subscriber_count(), 1, "one connection for two subscribers");

    sub_a.unsubscribe();
    assert_eq!(TestScheduler::pending_count(), 0, "subscribers remain, nothing is scheduled");
    TestScheduler::advance_by(Duration::from_millis(200));
    assert_eq!(source.inner.subscriber_count(), 1);

    sub_b.unsubscribe();
    TestScheduler::advance_by(GRACE);
    assert_eq!(source.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn ref_count_grace_with_uses_the_given_scheduler() {
    TestScheduler::init();

    // `Local`'s own scheduler would need a tokio `LocalSet`; the override must
    // be what schedules the reset.
    let source = Local::subject::<i32, Infallible>();
    let shared = source
      .clone()
      .publish()
      .ref_count_grace_with(GRACE, TestScheduler);

    shared.subscribe(|_: i32| {}).unsubscribe();
    assert_eq!(TestScheduler::pending_count(), 1, "the reset landed on the injected scheduler");
    assert_eq!(source.inner.subscriber_count(), 1);

    TestScheduler::advance_by(GRACE);
    assert_eq!(source.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn grace_works_in_a_shared_context() {
    type SharedTestCtx<T> = SharedCtx<T, TestScheduler>;

    TestScheduler::init();

    let mut source = SharedTestCtx::subject::<i32, Infallible>();
    let shared = source.clone().publish().ref_count_grace(GRACE);

    let log = Arc::new(Mutex::new(Vec::new()));
    let log_a = log.clone();
    let sub_a = shared
      .clone()
      .subscribe(move |v: i32| log_a.lock().unwrap().push(v));

    source.next(1);
    sub_a.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 1, "held open across the Arc/Mutex state");

    let log_b = log.clone();
    let _sub_b = shared.subscribe(move |v: i32| log_b.lock().unwrap().push(v));
    TestScheduler::advance_by(Duration::from_millis(100));

    assert_eq!(source.inner.subscriber_count(), 1, "the reset stood down");
    source.next(2);
    assert_eq!(*log.lock().unwrap(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn a_source_that_ends_during_the_window_is_still_released() {
    TestScheduler::init();

    let shared = TestCtx::from_iter([1, 2])
      .publish()
      .ref_count_grace(GRACE);
    let (a, b): (Log, Log) = (Default::default(), Default::default());

    // `from_iter` completes synchronously, so the connection is already dead
    // when the subscriber leaves; the reset must run without tripping over it.
    shared.clone().subscribe(sink(&a)).unsubscribe();
    TestScheduler::advance_by(GRACE);

    shared.subscribe(sink(&b));
    assert_eq!(*a.borrow(), vec![1, 2]);
    assert_eq!(*b.borrow(), vec![1, 2]);
  }
}
