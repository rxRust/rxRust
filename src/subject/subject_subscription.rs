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

pub(crate) struct RemoveState<P> {
  observers: P,
  id: usize,
}

impl<P, Sch, Cell> SubjectSubscription<P, Sch, Cell> {
  /// Requests cancellation and returns the removal task for internal callers
  /// that need to wait. The returned handle must not be cancelled.
  pub(crate) fn unsubscribe_inner<O>(self) -> TaskHandle
  where
    P: RcDerefMut<Target = Subscribers<O>>,
    Sch: Scheduler<Task<RemoveState<P>>>,
    Cell: SharedCell<SubscriptionState>,
  {
    loop {
      let current = self.state.get();
      if current == SubscriptionState::Cancelled {
        return TaskHandle::finished();
      }
      if self
        .state
        .compare_exchange(current, SubscriptionState::Cancelled)
        .is_err()
      {
        continue;
      }
      let SubscriptionState::Ready(id) = current else {
        // A pending add will skip insertion or roll it back under the list
        // lock.
        return TaskHandle::finished();
      };
      if let Some(mut guard) = self.observers.try_rc_deref_mut() {
        let observer = guard.remove(id);
        drop(guard);
        drop(observer);
        return TaskHandle::finished();
      }
      return self.scheduler.schedule(
        Task::new(RemoveState { observers: self.observers, id }, |state| {
          let observer = { state.observers.rc_deref_mut().remove(state.id) };
          drop(observer);
          TaskState::Finished
        }),
        None,
      );
    }
  }
}

impl<P, O, Sch, Cell> Subscription for SubjectSubscription<P, Sch, Cell>
where
  P: RcDerefMut<Target = Subscribers<O>>,
  Sch: Scheduler<Task<RemoveState<P>>>,
  Cell: SharedCell<SubscriptionState>,
{
  fn unsubscribe(self) { self.unsubscribe_inner(); }

  fn is_closed(&self) -> bool { self.state.get() == SubscriptionState::Cancelled }
}
