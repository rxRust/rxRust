//! Debounce Operator Implementation
//!
//! Emits an item from the source Observable only after a particular duration
//! has passed without another source emission.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskHandle, TaskState},
  subscription::{SourceWithHandle, Subscription},
};

// ==================== Debounce Operator ====================

/// Debounce operator that emits a value only after a quiet period.
///
/// Emits an item from the source Observable only after a particular duration
/// has passed without another source emission. Each new emission resets the
/// timer.
#[doc(alias = "debounceTime")]
#[derive(Clone)]
pub struct Debounce<S, Sch> {
  pub source: S,
  pub duration: Duration,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for Debounce<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== Debounce Subscription ====================

/// Subscription for the debounce operator
///
/// Uses `Option<TaskHandle>` directly as the handle type since it
/// already implements `Subscription`.
pub type DebounceSubscription<U, H> = SourceWithHandle<U, H>;

// ==================== Debounce Observer ====================

/// Observer that implements debounce logic
///
/// Type Parameters:
/// - `P`: Rc-wrapped Option of downstream observer
/// - `Sch`: Scheduler type
/// - `H`: Rc-wrapped Option<TaskHandle> for task handle management
/// - `V`: Rc-wrapped Option of trailing value
pub struct DebounceObserver<P, Sch, H, V> {
  pub observer: P,
  pub scheduler: Sch,
  pub duration: Duration,
  pub handle_state: H,
  pub trailing_value: V,
}

/// Task handler for emitting the debounced value
fn debounce_emit_handler<P, Item, Err, V>(task_state: &mut (P, V)) -> TaskState
where
  P: Observer<Item, Err>,
  V: RcDerefMut<Target = Option<Item>>,
{
  let (observer, value_rc) = task_state;

  // Take the trailing value
  if let Some(value) = value_rc.rc_deref_mut().take() {
    observer.next(value);
  }
  TaskState::Finished
}

impl<P, Sch, Item, Err, H, V> Observer<Item, Err> for DebounceObserver<P, Sch, H, V>
where
  P: Observer<Item, Err> + Clone,
  H: RcDerefMut<Target = Option<TaskHandle>>,
  V: RcDerefMut<Target = Option<Item>> + Clone,
  Sch: Scheduler<Task<(P, V)>>,
{
  fn next(&mut self, v: Item) {
    // Cancel any pending emission
    if let Some(handle) = self.handle_state.rc_deref_mut().take() {
      handle.unsubscribe();
    }

    // Store the new trailing value
    *self.trailing_value.rc_deref_mut() = Some(v);

    // Schedule new emission
    let task_state = (self.observer.clone(), self.trailing_value.clone());
    let task = Task::new(task_state, debounce_emit_handler::<P, Item, Err, V>);
    let handle = self.scheduler.schedule(task, Some(self.duration));
    *self.handle_state.rc_deref_mut() = Some(handle);
  }

  fn error(self, e: Err) {
    // Cancel pending emission and propagate error immediately
    if let Some(handle) = self.handle_state.rc_deref_mut().take() {
      handle.unsubscribe();
    }
    self.observer.error(e);
  }

  fn complete(self) {
    // Cancel pending scheduled task (we'll emit synchronously)
    if let Some(handle) = self.handle_state.rc_deref_mut().take() {
      handle.unsubscribe();
    }

    // Emit trailing value if any, then complete
    let trailing = self.trailing_value.rc_deref_mut().take();
    if let Some(value) = trailing {
      self.observer.clone().next(value);
    }
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

// ==================== CoreObservable Implementation ====================

impl<S, Sch, C, Unsub> CoreObservable<C> for Debounce<S, Sch>
where
  C: Context,
  C::RcMut<Option<TaskHandle>>: Subscription,
  S: for<'a> CoreObservable<
      C::With<
        DebounceObserver<
          C::RcMut<Option<C::Inner>>,
          Sch,
          C::RcMut<Option<TaskHandle>>,
          C::RcMut<Option<<S as ObservableType>::Item<'a>>>,
        >,
      >,
      Unsub = Unsub,
    >,
  Unsub: Subscription,
{
  type Unsub = DebounceSubscription<Unsub, C::RcMut<Option<TaskHandle>>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Debounce { source, duration, scheduler } = self;

    let handle_state: C::RcMut<Option<TaskHandle>> = C::RcMut::from(None);
    let trailing_value = C::RcMut::from(None);

    let wrapped = context.transform(|observer| DebounceObserver {
      observer: C::RcMut::from(Some(observer)),
      scheduler,
      duration,
      handle_state: handle_state.clone(),
      trailing_value,
    });

    let source_sub = source.subscribe(wrapped);

    SourceWithHandle::new(source_sub, handle_state)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::{
    sync::{Arc, Mutex},
    time::Duration,
  };

  use super::*;
  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_debounce_emits_last_after_quiet_period() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    // Interval emits every 20ms, debounce window is 30ms
    // So only the last value should be emitted (after take(5) completes)
    Local::interval(Duration::from_millis(20))
      .take(5)
      .debounce(Duration::from_millis(30))
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(200))
      .await;

    let result = values.lock().unwrap().clone();
    // Only the last value (4) should be emitted because each emission
    // cancels the previous pending one
    assert_eq!(result, vec![4]);
  }

  /// Test that debounce emits each value when spacing exceeds duration.
  /// Uses TestCtx for deterministic timing control.
  #[rxrust_macro::test]
  fn test_debounce_emits_each_when_spacing_exceeds_duration() {
    use std::{cell::RefCell, rc::Rc};

    use crate::{context::TestCtx, factory::ObservableFactory, prelude::TestScheduler};

    TestScheduler::init();

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .debounce(Duration::from_millis(20))
      .subscribe(move |v| {
        values_c.borrow_mut().push(v);
      });

    // Emit value 0, advance time past debounce window
    subject.next(0);
    TestScheduler::advance_by(Duration::from_millis(30));

    // Emit value 1, advance time past debounce window
    subject.next(1);
    TestScheduler::advance_by(Duration::from_millis(30));

    // Emit value 2 and complete
    subject.next(2);
    subject.complete();

    let result = values.borrow().clone();
    // All 3 values should be emitted since spacing exceeds debounce duration
    assert_eq!(result, vec![0, 1, 2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_debounce_complete_emits_trailing() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    // Source completes immediately after emitting, trailing value should be emitted
    Local::of(42)
      .debounce(Duration::from_millis(100))
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(20))
      .await;

    let result = values.lock().unwrap().clone();
    assert_eq!(result, vec![42]);
    assert!(*completed.lock().unwrap());
  }

  #[rxrust_macro::test(local)]
  async fn test_debounce_unsubscribe_cancels_pending() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    // Use a subject to control completion timing
    let mut subject = Local::subject::<i32, std::convert::Infallible>();

    let subscription = subject
      .clone()
      .debounce(Duration::from_millis(50))
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    // Emit value but don't complete
    subject.next(42);

    // Unsubscribe immediately before debounce timer fires
    subscription.unsubscribe();

    // Wait for what would have been the debounce time
    LocalScheduler
      .sleep(Duration::from_millis(100))
      .await;

    let result = values.lock().unwrap().clone();
    assert!(result.is_empty());
  }

  #[rxrust_macro::test]
  async fn test_debounce_with_shared_scheduler() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();
    let completed = Arc::new(Mutex::new(false));
    let completed_c = completed.clone();

    Shared::of(1)
      .merge(Shared::of(2))
      .merge(Shared::of(3))
      .debounce(Duration::from_millis(10))
      .on_complete(move || *completed_c.lock().unwrap() = true)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    // Wait for debounce to complete
    LocalScheduler
      .sleep(Duration::from_millis(50))
      .await;

    let result = values.lock().unwrap().clone();
    // Only the last merged value should be emitted
    assert_eq!(result.len(), 1);
    assert!(*completed.lock().unwrap());
  }
}
