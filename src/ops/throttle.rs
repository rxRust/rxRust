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
  subscription::{IntoBoxedSubscription, SingleAssignment, Subscription, single_assignment::State},
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
pub struct ThrottleSubscription<U, H> {
  source: U,
  handle: H,
}

impl<U, H> ThrottleSubscription<U, H> {
  #[inline]
  fn new(source: U, handle: H) -> Self { Self { source, handle } }
}

impl<U, H> Subscription for ThrottleSubscription<U, H>
where
  U: Subscription,
  H: Subscription,
{
  fn unsubscribe(self) {
    self.handle.unsubscribe();
    self.source.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.handle.is_closed() }
}

pub struct ThrottleState<Item, O, Param, BoxedSub> {
  // Downstream observer.
  observer: Option<O>,
  // Active throttle-window subscription.
  window: Option<BoxedSub>,
  generation: usize,
  // Pending value for trailing mode.
  pending: Option<Item>,
  // Source has completed while a window is active.
  completed: bool,
  // Notifier builder.
  param: Param,
  // Leading/trailing behavior.
  edge: ThrottleEdge,
}

pub struct ThrottleSubscriber<P, Item, H> {
  source: H,
  state: P,
  start_window_fn: fn(&Self, Item, bool),
}

impl<P, Item, H: Clone> Clone for ThrottleSubscriber<P, Item, H>
where
  P: Clone,
{
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      state: self.state.clone(),
      start_window_fn: self.start_window_fn,
    }
  }
}

impl<P, Item, O, Param, BoxedSub, Notifier, H: Subscription + Clone> ThrottleSubscriber<P, Item, H>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>> + Clone,
  Param: ThrottleParam<Item, Notifier = Notifier>,
  BoxedSub: Subscription,
  Notifier: Observable<
    Inner: CoreObservable<
      Notifier::With<ThrottleNotifyObserver<P, Item, WindowSlot<Notifier>, H>>,
      Unsub: IntoBoxedSubscription<Notifier::BoxedSubscription>,
    >,
  >,
  WindowSlot<Notifier>: IntoBoxedSubscription<BoxedSub>,
  O: Observer<Item, Notifier::Err> + Clone,
{
  fn new(state: P, source: H) -> Self {
    Self { state, source, start_window_fn: Self::start_window_impl }
  }

  fn start_window_impl(&self, value: Item, trailing_emission: bool) {
    let slot = WindowSlot::<Notifier>::new();
    let (notifier, previous, generation, emit) = {
      let mut guard = self.state.rc_deref_mut();
      let Some(inner) = guard.as_mut() else {
        return;
      };
      inner.generation += 1;
      let previous = inner.window.replace(slot.clone().into_boxed());
      let notifier = inner.param.notify_observable(&value);
      let emit = if trailing_emission || inner.edge.leading {
        Some(value)
      } else {
        if inner.edge.trailing {
          inner.pending = Some(value);
        }
        None
      };
      (notifier, previous, inner.generation, emit)
    };
    previous.unsubscribe();
    let (core, ctx) = notifier.swap(ThrottleNotifyObserver {
      subscriber: self.clone(),
      slot: slot.clone(),
      generation,
    });
    slot.set(core.subscribe(ctx).into_boxed());
    if let Some(value) = emit {
      let observer = {
        self
          .state
          .rc_deref()
          .as_ref()
          .and_then(|s| s.observer.clone())
      };
      if let Some(mut observer) = observer {
        observer.next(value);
      }
    }
  }
}

impl<P, Item, H> ThrottleSubscriber<P, Item, H> {
  fn start_window(&self, value: Item, trailing_emission: bool) {
    (self.start_window_fn)(self, value, trailing_emission);
  }
}

impl<P, Item, O, Param, BoxedSub, H: Subscription + Clone> ThrottleSubscriber<P, Item, H>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  BoxedSub: Subscription,
{
  fn close_window<Err>(&self, generation: usize)
  where
    O: Observer<Item, Err> + Clone,
  {
    let (retired, pending, completed, mut observer) = {
      let mut guard = self.state.rc_deref_mut();
      let Some(inner) = guard.as_mut() else {
        return;
      };
      if inner.generation != generation || inner.window.is_none() {
        return;
      }
      let result =
        (inner.window.take(), inner.pending.take(), inner.completed, inner.observer.clone());
      if inner.completed {
        *guard = None;
      }
      result
    };
    drop(retired);
    if let Some(pending) = pending {
      if completed {
        if let Some(observer) = observer.as_mut() {
          observer.next(pending);
        }
      } else {
        self.start_window(pending, true);
      }
    }
    if completed && let Some(observer) = observer {
      observer.complete();
    }
  }

  fn notifier_error<Err>(self, err: Err)
  where
    O: Observer<Item, Err> + Clone,
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

type WindowSlot<C> =
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>;
pub struct ThrottleNotifyObserver<P, Item, W, H> {
  subscriber: ThrottleSubscriber<P, Item, H>,
  slot: W,
  generation: usize,
}

pub struct ThrottleObserver<State, Item, H>(ThrottleSubscriber<State, Item, H>);

impl<State, Item, Err, O, Param, BoxedSub, H: Subscription + Clone> Observer<Item, Err>
  for ThrottleObserver<State, Item, H>
where
  State: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  Param: ThrottleParam<Item>,
  O: Observer<Item, Err> + Clone,
  BoxedSub: Subscription,
{
  fn next(&mut self, value: Item) {
    {
      let mut guard = self.0.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else {
        return;
      };
      if state.observer.is_closed() {
        return;
      }
      if state.window.is_some() {
        if state.edge.trailing {
          state.pending = Some(value);
        }
        return;
      }
    }
    self.0.start_window(value, false);
  }
  fn error(self, err: Err) { self.0.notifier_error(err); }
  fn complete(self) {
    let state = {
      let mut guard = self.0.state.rc_deref_mut();
      let Some(state) = guard.as_mut() else {
        return;
      };
      if state.window.is_some() && state.edge.trailing && state.pending.is_some() {
        state.completed = true;
        return;
      }
      guard.take()
    };
    if let Some(mut state) = state {
      state.window.unsubscribe();
      if state.edge.trailing
        && let (Some(pending), Some(observer)) = (state.pending.take(), state.observer.as_mut())
      {
        observer.next(pending);
      }
      if let Some(observer) = state.observer {
        observer.complete();
      }
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

impl<NotifyItem, P, Item, Err, O, Param, BoxedSub, W, H> Observer<NotifyItem, Err>
  for ThrottleNotifyObserver<P, Item, W, H>
where
  P: RcDerefMut<Target = Option<ThrottleState<Item, O, Param, BoxedSub>>>,
  O: Observer<Item, Err> + Clone,
  BoxedSub: Subscription,
  H: Subscription + Clone,
  W: Subscription + Clone,
{
  fn next(&mut self, _: NotifyItem) {
    self.subscriber.close_window(self.generation);
    self.slot.clone().unsubscribe();
  }
  fn error(self, err: Err) {
    let current = self
      .subscriber
      .state
      .rc_deref()
      .as_ref()
      .is_some_and(|s| s.generation == self.generation && s.window.is_some());
    if current {
      // Natural terminal notification retires this window without cancellation.
      let retired = {
        self
          .subscriber
          .state
          .rc_deref_mut()
          .as_mut()
          .and_then(|s| s.window.take())
      };
      drop(retired);
      let completed = self
        .subscriber
        .state
        .rc_deref()
        .as_ref()
        .is_none_or(|s| s.completed);
      if !completed {
        self.subscriber.source.clone().unsubscribe();
      }
      self.subscriber.notifier_error(err);
    }
  }
  fn complete(self) { self.subscriber.close_window(self.generation); }
  fn is_closed(&self) -> bool {
    self
      .subscriber
      .state
      .rc_deref()
      .as_ref()
      .is_none_or(|s| s.generation != self.generation || s.window.is_none())
  }
}

// ==================== CoreObservable Implementation ====================

/// Shared state handle type used by the throttle operator.
type RcThrottleState<C, Item, Param> = <C as Context>::RcMut<
  Option<
    ThrottleState<
      Item,
      <C as Context>::RcMut<Option<<C as Context>::Inner>>,
      Param,
      <C as Context>::BoxedSubscription,
    >,
  >,
>;

type ThrottleSourceObserverCtx<C, Param, Item, H> =
  <C as Context>::With<ThrottleObserver<RcThrottleState<C, Item, Param>, Item, H>>;
type SourceSlot<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;

type NotifierObserver<C, Param, Item, N, H> =
  ThrottleNotifyObserver<RcThrottleState<C, Item, Param>, Item, WindowSlot<N>, H>;

impl<S, Param, C, Unsub, Notifier> CoreObservable<C> for Throttle<S, Param>
where
  C: Context,
  Param: for<'a> ThrottleParam<<S as ObservableType>::Item<'a>, Notifier = Notifier>,
  S: for<'a> CoreObservable<
      ThrottleSourceObserverCtx<C, Param, <S as ObservableType>::Item<'a>, ()>,
      Unsub = Unsub,
    >,
  S: for<'a> CoreObservable<
      ThrottleSourceObserverCtx<C, Param, <S as ObservableType>::Item<'a>, SourceSlot<C, Unsub>>,
      Unsub = Unsub,
    >,
  Notifier: for<'a> Observable<
      Inner: CoreObservable<
        Notifier::With<
          NotifierObserver<
            C,
            Param,
            <S as ObservableType>::Item<'a>,
            Notifier,
            SourceSlot<C, Unsub>,
          >,
        >,
        Unsub: IntoBoxedSubscription<Notifier::BoxedSubscription>,
      >,
      Err = <S as ObservableType>::Err,
    >,
  for<'a> RcThrottleState<C, <S as ObservableType>::Item<'a>, Param>:
    IntoBoxedSubscription<C::BoxedSubscription>,
  WindowSlot<Notifier>: IntoBoxedSubscription<C::BoxedSubscription>,
  for<'a> C::RcMut<Option<C::Inner>>: Observer<S::Item<'a>, S::Err>,
  Unsub: Subscription,
{
  type Unsub = ThrottleSubscription<SourceSlot<C, Unsub>, C::BoxedSubscription>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Throttle { source, param, edge } = self;
    let source_slot = SourceSlot::<C, Unsub>::new();
    let state = C::RcMut::from(None);
    let state_handle = state.clone().into_boxed();

    let wrapped = context.transform(|observer| {
      *state.rc_deref_mut() = Some(ThrottleState {
        observer: Some(C::RcMut::from(Some(observer))),
        window: None,
        generation: 0,
        pending: None,
        completed: false,
        param,
        edge,
      });

      let subscriber = ThrottleSubscriber::new(state.clone(), source_slot.clone());
      ThrottleObserver(subscriber)
    });

    source_slot.set(source.subscribe(wrapped));
    ThrottleSubscription::new(source_slot, state_handle)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn synchronous_window_completion_emits_trailing_value() {
    use std::{cell::RefCell, rc::Rc};
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    Local::from_iter([1, 2])
      .throttle(|_: &i32| Local::empty(), ThrottleEdge::trailing())
      .subscribe(move |x| v.borrow_mut().push(x));
    assert_eq!(*values.borrow(), vec![1, 2]);
  }
  #[rxrust_macro::test]
  fn old_notifier_cannot_close_replacement_window() {
    use std::{cell::RefCell, rc::Rc};

    use crate::test_support::Manual;
    let source = Manual::default();
    let notifier = Manual::default();
    let n = notifier.clone();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    Local::new(source.clone())
      .throttle(move |_: &i32| Local::new(n.clone()), ThrottleEdge::leading())
      .on_error(|_| {})
      .subscribe(move |x| v.borrow_mut().push(x));
    source.next(0, 1);
    notifier.next(0, 0);
    source.next(0, 2);
    notifier.complete(0);
    source.next(0, 3);
    assert_eq!(*values.borrow(), vec![1, 2]);
    notifier.complete(1);
    source.next(0, 4);
    assert_eq!(*values.borrow(), vec![1, 2, 4]);
  }
  #[rxrust_macro::test]
  fn synchronous_notifier_completion_does_not_cancel_its_late_handle() {
    use std::{cell::Cell, rc::Rc};

    use crate::subscription::ClosureSubscription;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    let values = Rc::new(std::cell::RefCell::new(vec![]));
    let v = values.clone();
    Local::from_iter([1, 2])
      .throttle(
        move |_: &i32| {
          let c = c.clone();
          Local::create::<(), std::convert::Infallible, _, _>(move |e| {
            e.complete();
            ClosureSubscription(move || c.set(c.get() + 1))
          })
        },
        ThrottleEdge::leading(),
      )
      .subscribe(move |x| v.borrow_mut().push(x));
    assert_eq!(*values.borrow(), vec![1, 2]);
    assert_eq!(count.get(), 0);
  }
  #[rxrust_macro::test]
  fn notifier_error_cancels_source_and_callback_can_cancel() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{subscription::BoxedSubscription, test_support::Manual};
    let source = Manual::default();
    let notifier = Manual::default();
    let n = notifier.clone();
    Local::new(source.clone())
      .throttle(move |_: &i32| Local::new(n.clone()), ThrottleEdge::leading())
      .on_error(|_| {})
      .subscribe(|_| {});
    source.next(0, 1);
    notifier.error(0);
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(notifier.cancellations(0), 0);
    let source = Manual::default();
    let notifier = Manual::default();
    let n = notifier.clone();
    let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
    let h = holder.clone();
    let sub = Local::new(source.clone())
      .throttle(move |_: &i32| Local::new(n.clone()), ThrottleEdge::leading())
      .on_error(|_| {})
      .subscribe(move |_| h.borrow_mut().take().unwrap().unsubscribe());
    *holder.borrow_mut() = Some(BoxedSubscription::new(sub));
    source.next(0, 1);
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(notifier.cancellations(0), 1);
  }
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
  async fn test_throttle_trailing_subscription_stays_open_until_delayed_flush() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler, subscription::Subscription};

    TestScheduler::init();
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    let subscription = TestCtx::of(42)
      .throttle_time(Duration::from_millis(100), ThrottleEdge::trailing())
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| values_c.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(10));
    assert!(
      !subscription.is_closed(),
      "trailing throttle must stay open while the delayed trailing value is pending"
    );
    assert!(values.borrow().is_empty());
    assert!(!*completed.borrow());

    TestScheduler::advance_by(Duration::from_millis(120));

    assert_eq!(*values.borrow(), vec![42]);
    assert!(*completed.borrow());
    assert!(subscription.is_closed());
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
