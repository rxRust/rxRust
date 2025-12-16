//! # Delay Operator
//!
//! The `Delay` operator delays each emission from the source Observable by a
//! specified duration.
//!
//! This module demonstrates the Scheduler system integration by scheduling
//! delayed emission of values.
//!
//! ## Key Features
//! - **Scheduler dependency injection**: The `Context` carries the `Scheduler`
//!   implementation.
//! - **Task management**: Uses `Task<S>` for delayed execution.
//! - **Lifecycle management**: `TaskHandle`s are tracked to allow cancellation
//!   upon unsubscription.
//! - **Scheduler Agnostic**: Works with both `LocalScheduler`
//!   (synchronous/local delay) and `SharedScheduler` (async).

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Schedulable, Scheduler, Task, TaskHandle, TaskState},
  subscription::Subscription,
};

// ==================== Delay Operator ====================

/// Delay operator that delays each emission by a specified duration.
///
/// This struct represents the Observable returned by the `delay` operator.
/// It wraps the source observable and delays the propagation of `next`,
/// `error`, and `complete` notifications.
#[derive(Clone)]
pub struct Delay<S, Sch> {
  pub source: S,
  pub delay: Duration,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for Delay<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== Delay Subscription ====================

use crate::subscription::{DynamicSubscriptions, SourceWithDynamicSubs};

/// The `Subscription` type for the `Delay` operator.
pub type DelaySubscription<U, S> = SourceWithDynamicSubs<U, S>;

/// Type alias for the delay state (collection of pending task handles)
pub type DelayState = DynamicSubscriptions<TaskHandle>;

// ==================== Delay Observer ====================

/// Observer wrapper that handles delayed emission.
///
/// It intercepts `next`, `error`, and `complete` events from the source
/// and schedules them on the provided scheduler.
pub struct DelayObserver<P, Sch, R> {
  pub observer: P,
  pub delay: Duration,
  pub scheduler: Sch,
  pub state: R,
}

/// Static handler function for delayed `next` emissions.
///
/// The task state includes: (observer, item, state, id)
/// On completion, the task removes itself from the state using its id.
fn delay_next_handler<P, Item, Err, R>(task_state: &mut (P, Option<Item>, R, usize)) -> TaskState
where
  P: Observer<Item, Err>,
  R: RcDerefMut<Target = DelayState>,
{
  let (observer, item_opt, state, id) = task_state;

  if let Some(item) = item_opt.take() {
    observer.next(item);
  }

  state.rc_deref_mut().remove(*id);

  TaskState::Finished
}

/// Static handler function for delayed `complete` emissions.
fn delay_complete_handler<P, Item, Err, R>(task_state: &mut (P, R, usize)) -> TaskState
where
  P: Observer<Item, Err> + Clone,
  R: RcDerefMut<Target = DelayState>,
{
  let (observer, state, id) = task_state;

  state.rc_deref_mut().remove(*id);
  observer.clone().complete();

  TaskState::Finished
}

impl<P, Sch, Item, Err, R> Observer<Item, Err> for DelayObserver<P, Sch, R>
where
  P: Observer<Item, Err> + Clone,
  R: RcDerefMut<Target = DelayState> + Clone,
  Sch: Scheduler<Task<(P, Option<Item>, R, usize)>> + Scheduler<Task<(P, R, usize)>>,
  Task<(P, Option<Item>, R, usize)>: Schedulable<Sch>,
  Task<(P, R, usize)>: Schedulable<Sch>,
{
  fn next(&mut self, v: Item) {
    // Reserve ID first, then schedule task with that ID
    let id = self.state.rc_deref_mut().reserve_id();
    let task_state = (self.observer.clone(), Some(v), self.state.clone(), id);

    let task = Task::new(task_state, delay_next_handler::<P, Item, Err, R>);
    let handle = self.scheduler.schedule(task, Some(self.delay));

    self.state.rc_deref_mut().insert(id, handle);
  }

  fn error(self, e: Err) {
    for handle in self.state.rc_deref_mut().drain() {
      handle.unsubscribe();
    }

    self.observer.error(e);
  }

  fn complete(self) {
    // Reserve ID first, then schedule delayed complete
    let id = self.state.rc_deref_mut().reserve_id();
    let task_state = (self.observer.clone(), self.state.clone(), id);

    let task = Task::new(task_state, delay_complete_handler::<P, Item, Err, R>);
    let handle = self.scheduler.schedule(task, Some(self.delay));
    self.state.rc_deref_mut().insert(id, handle);
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

// ==================== CoreObservable Implementation for Delay
// ====================

impl<S, Sch, C> CoreObservable<C> for Delay<S, Sch>
where
  C: Context,
  // S must be able to work with our wrapped observer
  S: CoreObservable<C::With<DelayObserver<C::RcMut<Option<C::Inner>>, Sch, C::RcMut<DelayState>>>>,
{
  type Unsub = DelaySubscription<S::Unsub, C::RcMut<DelayState>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Delay { source, delay, scheduler } = self;

    let state = C::RcMut::from(DelayState::new());

    let wrapped = context.transform(|observer| DelayObserver {
      observer: C::RcMut::from(Some(observer)),
      scheduler,
      delay,
      state: state.clone(),
    });

    let source_sub = source.subscribe(wrapped);

    SourceWithDynamicSubs::new(source_sub, state)
  }
}

// ==================== DelaySubscription Operator ====================

/// Operator that delays the *subscription* to the source Observable.
///
/// When this observable is subscribed to, it waits for the specified duration
/// before actually subscribing to the source.
#[derive(Clone)]
pub struct DelaySubscriptionOp<S, Sch> {
  pub source: S,
  pub delay: Duration,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for DelaySubscriptionOp<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== DelaySubscription State ====================

use crate::subscription::EitherSubscription;

type DelaySubscriptionState<S, C, U> =
  (Option<S>, Option<C>, <C as Context>::RcMut<EitherSubscription<TaskHandle, U>>);

/// Static handler function for the delayed subscription task.
fn delay_subscription_handler<S, C, U>(state: &mut DelaySubscriptionState<S, C, U>) -> TaskState
where
  C: Context,
  S: CoreObservable<C, Unsub = U>,
  U: Subscription,
{
  let (source_opt, context_opt, subscription) = state;

  if let (Some(source), Some(context)) = (source_opt.take(), context_opt.take()) {
    let mut subscription = subscription.rc_deref_mut();
    if !subscription.is_closed() {
      let sub = source.subscribe(context);
      // Switch the subscription from the TaskHandle (waiting) to the actual source
      // subscription
      *subscription = EitherSubscription::Right(sub);
    }
  }
  TaskState::Finished
}

// ==================== CoreObservable Implementation for DelaySubscriptionOp
// ====================

impl<S, Sch, C> CoreObservable<C> for DelaySubscriptionOp<S, Sch>
where
  C: Context,
  S: CoreObservable<C>,
  C::RcMut<EitherSubscription<TaskHandle, S::Unsub>>: Subscription,
  Sch: Scheduler<Task<(Option<S>, Option<C>, C::RcMut<EitherSubscription<TaskHandle, S::Unsub>>)>>
    + Clone,
  Task<(Option<S>, Option<C>, C::RcMut<EitherSubscription<TaskHandle, S::Unsub>>)>:
    Schedulable<Sch>,
{
  type Unsub = C::RcMut<EitherSubscription<TaskHandle, S::Unsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let DelaySubscriptionOp { source, delay, scheduler } = self;

    let subscription = C::RcMut::from(EitherSubscription::Idle);

    let task_state = (Some(source), Some(context), subscription.clone());
    let task = Task::new(task_state, delay_subscription_handler);
    let handle = scheduler.schedule(task, Some(delay));

    // Store the task handle so it can be cancelled if the user unsubscribes before
    // the delay passes
    *subscription.rc_deref_mut() = EitherSubscription::Left(handle);
    subscription
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use super::*;
  use crate::{observable::of::Of, observer::FnMutObserver, prelude::*};

  #[rxrust_macro::test]
  fn test_delay_struct_creation() {
    // Test that Delay struct can be created properly
    use crate::scheduler::LocalScheduler;

    let source = Of(42);
    let delay = Delay { source, delay: Duration::from_millis(100), scheduler: LocalScheduler };

    // Verify struct fields
    assert_eq!(delay.delay, Duration::from_millis(100));
  }

  #[rxrust_macro::test]
  fn test_delay_observer_creation() {
    use crate::{context::MutRc, scheduler::LocalScheduler};

    let observer = |v: i32| println!("Got: {}", v);
    let state = MutRc::from(DelayState::new());

    let delay_observer = DelayObserver {
      observer,
      delay: Duration::from_millis(50),
      scheduler: LocalScheduler,
      state,
    };

    assert_eq!(delay_observer.delay, Duration::from_millis(50));
  }

  #[rxrust_macro::test]
  fn test_delay_handler() {
    use std::{cell::RefCell, rc::Rc};

    use crate::context::MutRc;

    let received = Rc::new(RefCell::new(None));
    let received_clone = received.clone();

    let observer = FnMutObserver(move |v: i32| {
      *received_clone.borrow_mut() = Some(v);
    });

    let inner_rc = MutRc::from(Some(observer));
    let state_rc: MutRc<DelayState> = MutRc::from(DelayState::new());

    // Reserve an ID and create task state
    let id = state_rc.rc_deref_mut().reserve_id();
    let mut task_state = (inner_rc, Some(42), state_rc.clone(), id);

    // Insert a dummy handle
    state_rc
      .rc_deref_mut()
      .insert(id, TaskHandle::finished());

    delay_next_handler::<_, i32, std::convert::Infallible, _>(&mut task_state);

    assert_eq!(*received.borrow(), Some(42));

    // Verify the handle was removed
    assert!(state_rc.rc_deref().is_empty());
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_method_signature() {
    // Test that delay method exists and has correct type signature
    let observable = Local::of(42);

    // This should compile without errors and return the right type
    let delayed = observable.delay(Duration::from_millis(100));

    // The type should be an Observable that can be subscribed to
    // If this compiles, the delay method is working correctly
    let _subscription = delayed.subscribe(|v| {
      println!("Received delayed value: {}", v);
    });
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_chaining() {
    // Test that delay can be chained with other operators
    let start = Instant::now();

    let _subscription = Local::of(1)
      .map(|x| x * 2)
      .delay(Duration::from_millis(10))
      .map(|x| x + 5)
      .on_complete(move || {
        let elapsed = start.elapsed();
        println!("Chain completed after {:?}", elapsed);
      })
      .subscribe(|v| {
        assert_eq!(v, 7); // (1 * 2) + 5 = 7
      });

    LocalScheduler
      .sleep(Duration::from_millis(50))
      .await;
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_integration() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_clone = values.clone();

    // Create an observable with TestScheduler-controlled timing
    TestCtx::of(1)
      .map(|x| x * 10)
      .delay(Duration::from_millis(50))
      .subscribe(move |v| values_clone.borrow_mut().push(v));

    // Advance virtual time to trigger delayed emission
    TestScheduler::advance_by(Duration::from_millis(80));

    // Verify we received the transformed value
    assert_eq!(values.borrow().clone(), vec![10]);
  }

  #[rxrust_macro::test]
  fn test_delay_compiles_with_other_operators() {
    // This test just verifies that delay operator can be chained
    // with other operators without compilation issues

    let _observable = Local::of(42)
      .map(|x| x + 1)
      .delay(Duration::from_millis(100))
      .on_complete(|| println!("Done!"));
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_at_method() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_clone = values.clone();

    TestCtx::of(1)
      .delay(Duration::from_millis(50))
      .subscribe(move |v| values_clone.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(80));

    assert_eq!(values.borrow().clone(), vec![1]);
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_with_scheduler() {
    use crate::scheduler::LocalScheduler;
    let start = Instant::now();
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_clone = values.clone();

    // Use a custom scheduler (explicitly passing LocalScheduler)
    let _subscription = Local::of(1)
      .delay_with(Duration::from_millis(50), LocalScheduler)
      .subscribe(move |v| values_clone.lock().unwrap().push(v));

    LocalScheduler
      .sleep(Duration::from_millis(80))
      .await;

    let elapsed = start.elapsed();
    assert_eq!(*values.lock().unwrap(), vec![1]);
    assert!(elapsed >= Duration::from_millis(40));
  }

  /// Test that delay cancellation prevents emission.
  /// Uses TestCtx for deterministic timing control.
  #[rxrust_macro::test]
  fn test_delay_cancellation() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{
      context::TestCtx, factory::ObservableFactory, observable::Observable, prelude::TestScheduler,
    };

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    // Observable that emits immediately, but delay holds it back
    let subscription = TestCtx::of(1)
      .delay(Duration::from_millis(50))
      .subscribe(move |v| values_c.borrow_mut().push(v));

    // Unsubscribe immediately
    subscription.unsubscribe();

    // Advance time past when the delayed emission would have occurred
    TestScheduler::advance_by(Duration::from_millis(100));

    // Should receive nothing because we unsubscribed
    assert!(values.borrow().is_empty());
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_subscription() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, prelude::TestScheduler};

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_clone = values.clone();

    // Create an observable that emits upon subscription, but subscription is
    // delayed
    TestCtx::of(1)
      .delay_subscription(Duration::from_millis(50))
      .subscribe(move |v| values_clone.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(80));

    // Verify we received the value
    assert_eq!(values.borrow().clone(), vec![1]);
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_subscription_cancellation() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_clone = values.clone();

    // Observable that emits immediately, but subscription is delayed
    let subscription = Local::of(1)
      .delay_subscription(Duration::from_millis(50))
      .subscribe(move |v| values_clone.lock().unwrap().push(v));

    subscription.unsubscribe();
    LocalScheduler
      .sleep(Duration::from_millis(100))
      .await;

    // Should receive nothing because we unsubscribed before subscription to source
    // occurred
    assert_eq!(*values.lock().unwrap(), Vec::<i32>::new());
  }

  #[rxrust_macro::test(local)]
  async fn test_delay_subscription_with_scheduler() {
    use crate::scheduler::LocalScheduler;
    let start = Instant::now();
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_clone = values.clone();

    // Use a custom scheduler (explicitly passing LocalScheduler)
    let _subscription = Local::of(1)
      .delay_subscription_with(Duration::from_millis(50), LocalScheduler)
      .subscribe(move |v| values_clone.lock().unwrap().push(v));

    LocalScheduler
      .sleep(Duration::from_millis(80))
      .await;

    let elapsed = start.elapsed();
    assert_eq!(*values.lock().unwrap(), vec![1]);
    assert!(elapsed >= Duration::from_millis(40));
  }
}
