use super::subscribers::Subscribers;
use crate::{
  context::{RcDerefMut, SharedCell},
  scheduler::{Scheduler, Task, TaskHandle, TaskState},
  subscription::Subscription,
};

/// State of the subscription.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubscriptionState {
  /// Waiting to be added to the subscribers list.
  Pending,
  /// Active with a specific ID in the subscribers list.
  Ready(usize),
  /// Cancelled (unsubscribed).
  Cancelled,
}

/// Subscription handle for a Subject.
///
/// This struct represents an active subscription to a Subject. When this handle
/// is explicitly unsubscribed, the corresponding observer is removed from the
/// Subject, possibly by a scheduled task. Ordinary Drop does not unsubscribe.
///
/// # Design
///
/// - **Shared Ownership**: Holds a reference-counted pointer to the Subject's
///   observers list.
/// - **State Tracking**: Uses a generic `SharedCell` to track the subscription
///   state (Pending, Ready(id), Cancelled) in a context-appropriate way.
/// - **No Borrow**: Does not borrow the Subject itself - uses shared ownership
///   instead.
///
/// # Type Parameters
///
/// - `P`: The smart pointer type (e.g., `Rc<RefCell<Subscribers<O>>>` or
///   `Arc<Mutex<Subscribers<O>>>`) Must implement `RcDerefMut` for unified
///   access to the inner `Subscribers`.
/// - `Sch`: The scheduler type.
/// - `Cell`: The shared cell type for storing `SubscriptionState`.
pub struct SubjectSubscription<P, Sch, Cell> {
  pub(crate) observers: P,
  pub(crate) state: Cell,
  pub(crate) scheduler: Sch,
}

impl<P, Sch, Cell> SubjectSubscription<P, Sch, Cell> {
  pub(crate) fn new(observers: P, state: Cell, scheduler: Sch) -> Self {
    Self { observers, state, scheduler }
  }
}

/// State of a scheduled Subject observer removal.
///
/// This type is public so custom operators can express the scheduler bounds of
/// [`SubjectSubscription::unsubscribe_with_handle`]. Its fields are managed by
/// Subject.
pub struct RemoveState<P> {
  observers: P,
  id: usize,
}

impl<P, Sch, Cell> SubjectSubscription<P, Sch, Cell> {
  /// Cancels this subscription and returns its observer-removal task handle.
  ///
  /// For a registered observer, the handle completes after removal from the
  /// Subject. If the list can be borrowed immediately, removal is synchronous
  /// and the handle is already finished. Otherwise, removal is scheduled on
  /// this subscription's scheduler, which must keep running while you await
  /// the handle. A pending registration is cancelled without waiting for its
  /// scheduled registration task.
  ///
  /// Dropping the returned handle does not cancel removal. Do not unsubscribe
  /// it or wrap it in an `unsubscribe_when_dropped()` guard: cancelling the
  /// task may prevent removal and makes the handle complete without
  /// performing it.
  ///
  /// ```
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// let subject = Local::subject::<(), Infallible>();
  /// let subscription = subject.clone().subscribe(|_| {});
  /// let removal = subscription.unsubscribe_with_handle();
  ///
  /// // Outside a broadcast, removal can finish immediately.
  /// assert!(removal.is_closed());
  /// assert!(subject.inner().is_empty());
  /// ```
  pub fn unsubscribe_with_handle<O>(self) -> TaskHandle
  where
    P: RcDerefMut<Target = Subscribers<O>>,
    Sch: Scheduler<Task<RemoveState<P>>>,
    Cell: SharedCell<SubscriptionState>,
  {
    let current = loop {
      let current = self.state.get();
      if current == SubscriptionState::Cancelled {
        return TaskHandle::finished();
      }
      if self
        .state
        .compare_exchange(current, SubscriptionState::Cancelled)
        .is_ok()
      {
        break current;
      }
    };
    let SubscriptionState::Ready(id) = current else {
      // A pending add will skip insertion or roll it back under the list lock.
      return TaskHandle::finished();
    };
    if let Some(mut guard) = self.observers.try_rc_deref_mut() {
      let observer = guard.remove(id);
      drop(guard);
      drop(observer);
      return TaskHandle::finished();
    }
    self.scheduler.schedule(
      Task::new(RemoveState { observers: self.observers, id }, |state| {
        let observer = { state.observers.rc_deref_mut().remove(state.id) };
        drop(observer);
        TaskState::Finished
      }),
      None,
    )
  }
}

impl<P, O, Sch, Cell> Subscription for SubjectSubscription<P, Sch, Cell>
where
  P: RcDerefMut<Target = Subscribers<O>>,
  Sch: Scheduler<Task<RemoveState<P>>>,
  Cell: SharedCell<SubscriptionState>,
{
  fn unsubscribe(self) { self.unsubscribe_with_handle(); }

  fn is_closed(&self) -> bool { self.state.get() == SubscriptionState::Cancelled }
}
