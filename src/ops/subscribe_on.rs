//! SubscribeOn operator implementation
//!
//! This module contains the SubscribeOn operator, which schedules the
//! subscription logic on a specified scheduler.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  scheduler::{Scheduler, Task, TaskHandle, TaskState},
};

/// SubscribeOn operator: Schedules the subscription logic on a specified
/// Scheduler.
///
/// Unlike `ObserveOn` which affects where notifications are delivered,
/// `SubscribeOn` affects where the source observable's subscription logic runs.
#[derive(Clone)]
pub struct SubscribeOn<S, Sch> {
  pub source: S,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for SubscribeOn<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

fn subscribe_task<S, C>(state: &mut Option<(S, C)>) -> TaskState
where
  S: CoreObservable<C>,
{
  if let Some((source, context)) = state.take() {
    source.subscribe(context);
  }
  TaskState::Finished
}

impl<C, S, Sch> CoreObservable<C> for SubscribeOn<S, Sch>
where
  C: Context,
  S: CoreObservable<C> + 'static,
  Sch: Scheduler<Task<Option<(S, C)>>>,
{
  type Unsub = TaskHandle;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SubscribeOn { source, scheduler } = self;
    let task = Task::new(Some((source, context)), subscribe_task::<S, C>);
    scheduler.schedule(task, None)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use crate::{prelude::*, scheduler::LocalScheduler};

  #[rxrust_macro::test(local)]
  async fn smoke_test_local() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_c = values.clone();

    Local::from_iter(0..5)
      .subscribe_on(LocalScheduler)
      .subscribe(move |v| {
        values_c.lock().unwrap().push(v);
      });

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*values.lock().unwrap(), vec![0, 1, 2, 3, 4]);
  }

  #[rxrust_macro::test(local)]
  async fn test_cancellation_before_execution() {
    let executed = Arc::new(Mutex::new(false));
    let executed_c = executed.clone();

    // Schedule subscription with a delay
    let sub = Local::from_iter([1, 2, 3])
      .delay_subscription(Duration::from_millis(50))
      .subscribe_on(LocalScheduler)
      .subscribe(move |_| {
        *executed_c.lock().unwrap() = true;
      });

    // Cancel immediately before it executes
    sub.unsubscribe();

    LocalScheduler
      .sleep(Duration::from_millis(120))
      .await;

    // Should not have executed
    assert!(!*executed.lock().unwrap());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  async fn test_shared_scheduler_different_thread() {
    use std::thread;

    let observed_thread = Arc::new(Mutex::new(None));
    let observed_c = observed_thread.clone();

    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));

    Shared::from_iter([1])
      .subscribe_on(SharedScheduler)
      .on_complete(move || {
        if let Some(tx) = tx.lock().unwrap().take() {
          let _ = tx.send(());
        }
      })
      .subscribe(move |_| {
        *observed_c.lock().unwrap() = Some(thread::current().id());
      });

    // Wait for completion
    let _ = rx.await;

    let obs_id = observed_thread.lock().unwrap().take();
    assert!(obs_id.is_some());
    // With SharedScheduler, task may run on a different thread
    // (depends on tokio runtime, but usually does)
    // Just verify the subscription worked
  }
}
