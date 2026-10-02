//! Timeout operator implementation
//!
//! Errors if the source stays silent for longer than a duration.

use std::fmt;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskHandle, TaskState},
  subscription::{SourceWithHandle, Subscription},
};

/// The error `timeout` emits when no item arrives in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeoutError;

impl fmt::Display for TimeoutError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("observable timed out") }
}

impl std::error::Error for TimeoutError {}

/// Builds the default timeout error for any `Err: From<TimeoutError>`.
pub fn default_timeout_error<E: From<TimeoutError>>() -> E { TimeoutError.into() }

/// Timeout operator: Error if the source is silent for `duration`
///
/// The timer starts at subscription and restarts after every item. When it
/// fires, the downstream receives the error from `error_fn` and the source
/// is released.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(not(target_arch = "wasm32"))]
/// # {
/// use rxrust::prelude::*;
///
/// # #[tokio::main(flavor = "local")]
/// # async fn main() {
/// Local::never()
///   .map_to(0)
///   .map_err(|_: std::convert::Infallible| TimeoutError)
///   .timeout(Duration::from_millis(50))
///   .on_error(|e| println!("{}", e))
///   .subscribe(|_| {});
/// # }
/// # }
/// ```
#[derive(Clone)]
pub struct Timeout<S, Sch, F> {
  pub source: S,
  pub duration: Duration,
  pub scheduler: Sch,
  pub error_fn: F,
}

impl<S, Sch, F> ObservableType for Timeout<S, Sch, F>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Subscription for the timeout operator
pub type TimeoutSubscription<U, H> = SourceWithHandle<U, H>;

/// Observer that arms and re-arms the timer
pub struct TimeoutObserver<P, Sch, H, E> {
  observer: P,
  scheduler: Sch,
  duration: Duration,
  handle_state: H,
  error_fn: E,
}

fn timeout_fire<P, E, O, F, Item, Err>(state: &mut (P, E)) -> TaskState
where
  P: RcDerefMut<Target = Option<O>>,
  E: RcDerefMut<Target = Option<F>>,
  O: Observer<Item, Err>,
  F: FnOnce() -> Err,
{
  let (observer_rc, error_rc) = state;
  let observer = observer_rc.rc_deref_mut().take();
  let error_fn = error_rc.rc_deref_mut().take();
  if let (Some(observer), Some(error_fn)) = (observer, error_fn) {
    observer.error(error_fn());
  }
  TaskState::Finished
}

impl<P, Sch, H, E, O, F> TimeoutObserver<P, Sch, H, E>
where
  P: RcDerefMut<Target = Option<O>> + Clone,
  E: RcDerefMut<Target = Option<F>> + Clone,
  H: RcDerefMut<Target = Option<TaskHandle>>,
  Sch: Scheduler<Task<(P, E)>>,
{
  fn arm<Item, Err>(&self)
  where
    O: Observer<Item, Err>,
    F: FnOnce() -> Err,
  {
    self.disarm();
    let task = Task::new(
      (self.observer.clone(), self.error_fn.clone()),
      timeout_fire::<P, E, O, F, Item, Err>,
    );
    let handle = self.scheduler.schedule(task, Some(self.duration));
    *self.handle_state.rc_deref_mut() = Some(handle);
  }

  fn disarm(&self) {
    if let Some(handle) = self.handle_state.rc_deref_mut().take() {
      handle.unsubscribe();
    }
  }
}

impl<P, Sch, H, E, O, F, Item, Err> Observer<Item, Err> for TimeoutObserver<P, Sch, H, E>
where
  P: RcDerefMut<Target = Option<O>> + Clone,
  E: RcDerefMut<Target = Option<F>> + Clone,
  H: RcDerefMut<Target = Option<TaskHandle>>,
  O: Observer<Item, Err>,
  F: FnOnce() -> Err,
  Sch: Scheduler<Task<(P, E)>>,
{
  fn next(&mut self, value: Item) {
    self.disarm();
    if let Some(observer) = self.observer.rc_deref_mut().as_mut() {
      observer.next(value);
    }
    if self.observer.rc_deref().is_some() {
      self.arm::<Item, Err>();
    }
  }

  fn error(self, err: Err) {
    self.disarm();
    if let Some(observer) = self.observer.rc_deref_mut().take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    self.disarm();
    if let Some(observer) = self.observer.rc_deref_mut().take() {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .observer
      .rc_deref()
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

type ObserverRc<C> = <C as Context>::RcMut<Option<<C as Context>::Inner>>;
type ErrorFnRc<C, F> = <C as Context>::RcMut<Option<F>>;
type HandleRc<C> = <C as Context>::RcMut<Option<TaskHandle>>;

impl<S, Sch, F, C> CoreObservable<C> for Timeout<S, Sch, F>
where
  C: Context,
  HandleRc<C>: Subscription,
  S: CoreObservable<C::With<TimeoutObserver<ObserverRc<C>, Sch, HandleRc<C>, ErrorFnRc<C, F>>>>,
  Sch: Scheduler<Task<(ObserverRc<C>, ErrorFnRc<C, F>)>>,
  C::Inner: for<'a> Observer<S::Item<'a>, S::Err>,
  F: FnOnce() -> S::Err,
{
  type Unsub = TimeoutSubscription<S::Unsub, HandleRc<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Timeout { source, duration, scheduler, error_fn } = self;
    let handle_state: HandleRc<C> = C::RcMut::from(None);
    let handle_for_observer = handle_state.clone();

    let wrapped = context.transform(|observer| {
      let timeout_observer = TimeoutObserver {
        observer: C::RcMut::from(Some(observer)),
        scheduler,
        duration,
        handle_state: handle_for_observer,
        error_fn: C::RcMut::from(Some(error_fn)),
      };
      timeout_observer.arm::<S::Item<'_>, S::Err>();
      timeout_observer
    });

    let source_sub = source.subscribe(wrapped);
    SourceWithHandle::new(source_sub, handle_state)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::TimeoutError;
  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  #[rxrust_macro::test]
  fn test_timeout_errors_when_silent() {
    TestScheduler::init();
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    let mut subject = TestCtx::subject::<i32, TimeoutError>();
    let _sub = subject
      .clone()
      .timeout(Duration::from_millis(100))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    subject.next(1);
    TestScheduler::advance_by(Duration::from_millis(60));
    assert!(error.borrow().is_none());
    // The item above reset the timer, so 60 + 60 > 100 fires now
    TestScheduler::advance_by(Duration::from_millis(60));
    assert_eq!(*error.borrow(), Some(TimeoutError));
  }

  #[rxrust_macro::test]
  fn test_timeout_items_keep_it_alive() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(None));
    let result_c = result.clone();
    let error_c = error.clone();

    let mut subject = TestCtx::subject::<i32, TimeoutError>();
    let _sub = subject
      .clone()
      .timeout(Duration::from_millis(100))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    for i in 0..5 {
      TestScheduler::advance_by(Duration::from_millis(50));
      subject.next(i);
    }

    assert_eq!(*result.borrow(), vec![0, 1, 2, 3, 4]);
    assert!(error.borrow().is_none());
  }

  #[rxrust_macro::test]
  fn test_timeout_or_else_custom_error_and_completion_cancels() {
    TestScheduler::init();
    let error = Rc::new(RefCell::new(None));
    let completed = Rc::new(RefCell::new(false));
    let error_c = error.clone();
    let completed_c = completed.clone();

    let subject = TestCtx::subject::<i32, String>();
    let _sub = subject
      .clone()
      .timeout_or_else(Duration::from_millis(10), || "late".to_string())
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(|_| {});

    subject.clone().complete();
    TestScheduler::advance_by(Duration::from_millis(50));

    assert!(*completed.borrow());
    assert!(error.borrow().is_none());
  }

  #[rxrust_macro::test]
  fn test_timeout_unsubscribe_cancels_timer() {
    TestScheduler::init();
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let subject = TestCtx::subject::<i32, TimeoutError>();
    let sub = subject
      .clone()
      .timeout(Duration::from_millis(10))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    sub.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(50));
    assert!(error.borrow().is_none());
  }
}
