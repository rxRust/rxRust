//! Throttle operator.
//!
//! This throttle is controlled by a *notifier* observable.
//!
//! Rules:
//! - A value starts a throttle window.
//! - While the window is active, new values are suppressed.
//! - If `trailing` is on, the last suppressed value is emitted when the window
//!   ends.
//! - If a trailing value is emitted, it starts the next window (keeps spacing).
//! - If the source completes during an active window and a trailing value is
//!   pending, completion waits until the window ends and the trailing value is
//!   emitted.

use crate::{
  Observable, Timer,
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::Duration,
  subscription::{IntoBoxedSubscription, SourceWithHandle, Subscription},
};

// ===== ThrottleEdge =====·

/// Controls when values are emitted.
///
/// - `leading`: emit the first value when a window starts.
/// - `trailing`: emit the last value when the window ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottleEdge {
  pub leading: bool,
  pub trailing: bool,
}

impl ThrottleEdge {
  /// Emit only the first value of each window.
  #[inline]
  pub fn leading() -> Self { Self { leading: true, trailing: false } }

  /// Emit only the last value when the window ends.
  #[inline]
  pub fn trailing() -> Self { Self { leading: false, trailing: true } }

  /// Emit both the first and the last value of each window.
  #[inline]
  pub fn all() -> Self { Self { leading: true, trailing: true } }
}

// ===== Throttle operator =====

/// Builds a notifier observable for each source value.
///
/// The returned observable controls the current throttle window:
/// the window is considered active until the notifier emits or completes.
pub trait ThrottleParam<Item> {
  type Notifier: Observable;
  fn notify_observable(&mut self, value: &Item) -> Self::Notifier;
}

/// Notifier-based throttle parameter.
#[doc(hidden)]
#[derive(Clone)]
pub struct ThrottleWhenParam<F> {
  pub selector: F,
}

impl<Item, F, Out> ThrottleParam<Item> for ThrottleWhenParam<F>
where
  F: FnMut(&Item) -> Out,
  Out: Observable,
{
  type Notifier = Out;

  fn notify_observable(&mut self, value: &Item) -> Self::Notifier { (self.selector)(value) }
}

impl<Item, C> ThrottleParam<Item> for C
where
  C: Context<Inner = Duration>,
{
  type Notifier = C::With<Timer<C::Scheduler>>;

  fn notify_observable(&mut self, _value: &Item) -> Self::Notifier {
    self.wrap(Timer { delay: *self.inner(), scheduler: self.scheduler().clone() })
  }
}

/// Throttle operator (core implementation).
///
/// Users typically construct this via extension methods like `throttle` or
/// `throttle_time`.
#[derive(Clone)]
pub struct Throttle<S, Param> {
  pub source: S,
  pub param: Param,
  pub edge: ThrottleEdge,
}

impl<S, Param> ObservableType for Throttle<S, Param>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ===== Subscription/state =====

/// Subscription for the throttle operator.
pub type ThrottleSubscription<U, H> = SourceWithHandle<U, H>;

pub struct ThrottleState<Item, O, Param, BoxedSub> {
  // Downstream observer.
  observer: Option<O>,
  // Active throttle-window subscription.
  window: Option<BoxedSub>,
  // Pending value for trailing mode.
  pending: Option<Item>,
  // Source has completed while a window is active.
  completed: bool,
  // Notifier builder.
  param: Param,
  // Leading/trailing behavior.
  edge: ThrottleEdge,
}

pub struct ThrottleSubscriber<P, Item> {
  state: P,
  start_window_fn: fn(&Self, &Item),
}

impl<P, Item> Clone for ThrottleSubscriber<P, Item>
where
  P: Clone,
{
  fn clone(&self) -> Self {
    Self { state: self.state.clone(), start_window_fn: self.start_window_fn }
  }
}

impl<P, Item, O, Param, BoxedSub, Notifier> ThrottleSubscriber<P, Item>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>> + Clone,
  Param: ThrottleParam<Item, Notifier = Notifier>,
  BoxedSub: Subscription,
  Notifier: Observable<
    Inner: CoreObservable<
      Notifier::With<ThrottleNotifyObserver<P, Item>>,
      Unsub: IntoBoxedSubscription<BoxedSub>,
    >,
  >,
{
  fn new(state: P) -> Self { Self { state, start_window_fn: Self::start_window_impl } }

  fn start_window_impl(&self, value: &Item) {
    // Do not hold the mutable borrow of `state` across `subscribe()`.
    // The notifier may synchronously call back into this operator (re-entrancy),
    // which would otherwise cause a runtime borrow panic.
    let notifier = {
      let mut guard = self.state.rc_deref_mut();
      let Some(inner) = guard.as_mut() else { return };
      let prev = inner.window.take();
      if let Some(prev) = prev {
        prev.unsubscribe();
      }
      inner.param.notify_observable(value)
    };

    // Always cancel the previous window before starting a new one.

    let (core, ctx) = notifier.swap(ThrottleNotifyObserver(self.clone()));
    let window = core.subscribe(ctx).into_boxed();

    // If the notifier completes synchronously, don't keep a closed window.
    // Otherwise `window.is_some()` would incorrectly suppress subsequent values
    // and could also make `complete()` think a window is still active.
    if window.is_closed() {
      window.unsubscribe();
      return;
    }

    let mut guard = self.state.rc_deref_mut();
    let Some(inner) = guard.as_mut() else {
      window.unsubscribe();
      return;
    };

    inner.window = Some(window);
  }
}

impl<P, Item> ThrottleSubscriber<P, Item> {
  fn start_window(&self, value: &Item) { (self.start_window_fn)(self, value); }
}

impl<P, Item, O, Param, BoxedSub> ThrottleSubscriber<P, Item>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  BoxedSub: Subscription,
{
  fn close_window<Err>(&self)
  where
    O: Observer<Item, Err>,
  {
    let mut guard = self.state.rc_deref_mut();
    let Some(inner) = guard.as_mut() else { return };

    if let Some(w) = inner.window.take() {
      w.unsubscribe();
    }

    let pending = inner.pending.take();

    // Spacing guarantee: if we will emit a trailing value and the source has
    // not completed, that trailing emission starts the next window.
    match (pending, inner.completed) {
      (Some(pending), false) => {
        // Start the next window before emitting, so spacing is guaranteed.
        drop(guard);
        self.start_window(&pending);

        let mut guard = self.state.rc_deref_mut();
        let Some(inner) = guard.as_mut() else { return };
        if let Some(observer) = inner.observer.as_mut() {
          observer.next(pending);
        }
      }
      (pending, completed) => {
        // If we don't need to start the next window, we can flush/complete while
        // holding the guard.
        if let (Some(p), Some(observer)) = (pending, inner.observer.as_mut()) {
          observer.next(p);
        }

        if completed && let Some(observer) = inner.observer.take() {
          observer.complete();
        }
      }
    }
  }

  fn notifier_error<Err>(self, err: Err)
  where
    O: Observer<Item, Err>,
  {
    let Some(mut inner) = self.state.rc_deref_mut().take() else { return };

    if let Some(w) = inner.window.take() {
      w.unsubscribe();
    }
    inner.pending.take();
    if let Some(observer) = inner.observer.take() {
      observer.error(err);
    }
  }
}

pub struct ThrottleNotifyObserver<P, Item>(ThrottleSubscriber<P, Item>);

pub struct ThrottleObserver<State, Item>(ThrottleSubscriber<State, Item>);

impl<State, Item, Err, O, Param, BoxedSub> Observer<Item, Err> for ThrottleObserver<State, Item>
where
  State: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  Param: ThrottleParam<Item>,
  O: Observer<Item, Err>,
  BoxedSub: Subscription,
{
  fn next(&mut self, value: Item) {
    let mut guard = self.0.state.rc_deref_mut();
    let Some(state) = guard.as_mut() else { return };

    if state.observer.is_closed() {
      return;
    }

    if state.window.is_none() {
      drop(guard);
      self.0.start_window(&value);

      let mut guard = self.0.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else { return };

      let edge = state.edge;
      if edge.leading {
        if let Some(observer) = state.observer.as_mut() {
          observer.next(value);
        }
      } else if edge.trailing {
        state.pending = Some(value);
      }
    } else if state.edge.trailing {
      state.pending = Some(value);
    }
  }

  fn error(self, err: Err) { self.0.notifier_error(err); }

  fn complete(self) {
    let mut guard = self.0.state.rc_deref_mut();
    let Some(state) = guard.as_mut() else { return };

    if state.window.is_some() && state.edge.trailing && state.pending.is_some() {
      state.completed = true;
      return;
    }

    if let Some(w) = state.window.take() {
      w.unsubscribe();
    }
    if state.edge.trailing
      && let (Some(pending), Some(observer)) = (state.pending.take(), state.observer.as_mut())
    {
      observer.next(pending);
    }
    if let Some(observer) = state.observer.take() {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .0
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|sub| sub.observer.is_closed())
  }
}

impl<Item, O, Param, BoxedSub> Subscription for ThrottleState<Item, O, Param, BoxedSub>
where
  Param: ThrottleParam<Item>,
  BoxedSub: Subscription,
{
  fn unsubscribe(mut self) {
    if let Some(w) = self.window.take() {
      w.unsubscribe();
    }
    self.pending.take();
    self.observer.take();
  }

  fn is_closed(&self) -> bool { self.observer.is_none() }
}

impl<NotifyItem, P, Item, Err, O, Param, BoxedSub> Observer<NotifyItem, Err>
  for ThrottleNotifyObserver<P, Item>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  O: Observer<Item, Err>,
  BoxedSub: Subscription,
{
  fn next(&mut self, _value: NotifyItem) { self.0.close_window(); }

  fn error(self, err: Err) { self.0.notifier_error(err); }

  fn complete(self) { self.0.close_window(); }

  fn is_closed(&self) -> bool { self.0.state.rc_deref().is_none() }
}

// ==================== CoreObservable Implementation ====================

/// Shared state handle type used by the throttle operator.
type RcThrottleState<C, Item, Param> = <C as Context>::RcMut<
  Option<ThrottleState<Item, <C as Context>::Inner, Param, <C as Context>::BoxedSubscription>>,
>;

type ThrottleSourceObserverCtx<C, Param, Item> =
  <C as Context>::With<ThrottleObserver<RcThrottleState<C, Item, Param>, Item>>;

type NotifierObserver<C, Param, Item> =
  ThrottleNotifyObserver<RcThrottleState<C, Item, Param>, Item>;

impl<S, Param, C, Unsub, Notifier> CoreObservable<C> for Throttle<S, Param>
where
  C: Context,
  Param: for<'a> ThrottleParam<<S as ObservableType>::Item<'a>, Notifier = Notifier>,
  S: for<'a> CoreObservable<
      ThrottleSourceObserverCtx<C, Param, <S as ObservableType>::Item<'a>>,
      Unsub = Unsub,
    >,
  Notifier: for<'a> Observable<
      Inner: CoreObservable<
        Notifier::With<NotifierObserver<C, Param, <S as ObservableType>::Item<'a>>>,
        Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
      >,
      Err = <S as ObservableType>::Err,
    >,
  for<'a> RcThrottleState<C, <S as ObservableType>::Item<'a>, Param>:
    IntoBoxedSubscription<C::BoxedSubscription>,
  Unsub: Subscription,
{
  type Unsub = ThrottleSubscription<Unsub, C::BoxedSubscription>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Throttle { source, param, edge } = self;
    let state = C::RcMut::from(None);
    let state_handle = state.clone().into_boxed();

    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() = Some(ThrottleState {
        observer: Some(observer),
        window: None,
        pending: None,
        completed: false,
        param,
        edge,
      });

      let subscriber = ThrottleSubscriber::new(state.clone());
      ThrottleObserver(subscriber)
    });

    let source_unsub = source.subscribe(wrapped);
    SourceWithHandle::new(source_unsub, state_handle)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;
  use crate::prelude::*;

  /// Leading mode emits immediately at start of each window.
  /// Uses TestScheduler for deterministic timing control.
  #[rxrust_macro::test]
  async fn test_throttle_leading() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, factory::ObservableFactory, prelude::TestScheduler};

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .throttle_time(Duration::from_millis(50), ThrottleEdge::leading())
      .subscribe(move |v| values_c.borrow_mut().push(v));

    // Emit values at specific times
    subject.next(0); // Emits 0 immediately, start window until 50ms
    assert_eq!(*values.borrow(), vec![0]);

    TestScheduler::advance_by(Duration::from_millis(20));
    subject.next(1); // Ignored (in window)
    TestScheduler::advance_by(Duration::from_millis(20));
    subject.next(2); // Ignored (in window)

    // Wait for window to expire (need to reach 50ms total)
    TestScheduler::advance_by(Duration::from_millis(10));

    // Now window has expired, start new window
    subject.next(3); // Emits 3, start new window
    assert_eq!(*values.borrow(), vec![0, 3]);

    TestScheduler::advance_by(Duration::from_millis(20));
    subject.next(4); // Ignored (in window)

    subject.complete();

    let result = values.borrow().clone();
    // Leading: emits immediately at start of each window
    assert_eq!(result, vec![0, 3]);
  }

  #[rxrust_macro::test]
  async fn test_throttle_trailing_completion_delayed() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();

    subject
      .clone()
      .throttle_time(Duration::from_millis(80), ThrottleEdge::trailing())
      .subscribe(move |v| values_c.borrow_mut().push(v));

    // Emit values at specific times
    subject.next(0); // Start window, pending=0, timer expires at 80ms
    TestScheduler::advance_by(Duration::from_millis(20));
    subject.next(1); // Update trailing=1
    TestScheduler::advance_by(Duration::from_millis(20));
    subject.next(2); // Update trailing=2

    // Wait for first window to end and trailing to be emitted.
    // (We also allow time for the new window started by the trailing emission.)
    TestScheduler::advance_by(Duration::from_millis(60));

    // During the new window (started by trailing emission), update pending.
    subject.next(3); // pending=3
    TestScheduler::advance_by(Duration::from_millis(10));
    subject.next(4); // Update pending=4

    // Completion semantics B: completion is delayed until window ends.
    subject.complete();

    // Wait for window to end and flush trailing value.
    TestScheduler::advance_by(Duration::from_millis(100));

    let result = values.borrow().clone();
    // Should emit: 2 (after first window), 4 (after second window ends)
    assert_eq!(result, vec![2, 4]);
  }

  #[rxrust_macro::test]
  async fn test_throttle_complete_waits_for_window_then_emits_trailing() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    TestCtx::of(42)
      .throttle_time(Duration::from_millis(100), ThrottleEdge::trailing())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| values_c.borrow_mut().push(v));

    // Source completes immediately, but trailing emission and completion are
    // delayed until the window ends.
    TestScheduler::advance_by(Duration::from_millis(120));

    let result = values.borrow().clone();
    // Window end should emit trailing value, then complete.
    assert_eq!(result, vec![42]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  async fn test_throttle_with_notifier_selector() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    TestCtx::interval(Duration::from_millis(50))
      .take(5)
      .throttle(
        |val: &usize| {
          let d = if val.is_multiple_of(2) {
            Duration::from_millis(115)
          } else {
            Duration::from_millis(55)
          };
          TestCtx::timer(d)
        },
        ThrottleEdge::leading(),
      )
      .subscribe(move |v| values_c.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(350));

    let result = values.borrow().clone();
    // Dynamic throttle based on notifier derived from value
    assert_eq!(result, vec![0, 3]);
  }

  #[rxrust_macro::test]
  async fn test_throttle_unsubscribe_cancels() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();

    let subscription = subject
      .clone()
      .throttle_time(Duration::from_millis(50), ThrottleEdge::trailing())
      .subscribe(move |v| values_c.borrow_mut().push(v));

    subject.next(42);
    subscription.unsubscribe();

    // Wait for what would have been the throttle time
    TestScheduler::advance_by(Duration::from_millis(120));

    let result = values.borrow().clone();
    assert!(result.is_empty());
  }

  #[rxrust_macro::test]
  async fn test_throttle_shared() {
    use std::sync::{Arc, Mutex};

    use crate::{context::SharedCtx, prelude::TestScheduler};

    TestScheduler::init();
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    type SharedTestCtx<T> = SharedCtx<T, TestScheduler>;

    SharedTestCtx::of(1)
      .merge(SharedTestCtx::of(2))
      .merge(SharedTestCtx::of(3))
      .throttle_time(Duration::from_millis(50), ThrottleEdge::leading())
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    // Wait for completion
    TestScheduler::advance_by(Duration::from_millis(100));

    let result = values.lock().unwrap().clone();
    // Leading: only first value should be emitted immediately.
    assert!(!result.is_empty());
    assert!(*completed.lock().unwrap());
  }
}
