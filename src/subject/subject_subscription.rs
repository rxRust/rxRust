use super::subscribers::Subscribers;
use crate::{
  context::{RcDerefMut, SharedCell},
  scheduler::{Scheduler, Task, TaskState},
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
/// is dropped or unsubscribed, the corresponding observer is removed from the
/// Subject.
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

impl<P, O, Sch, Cell> Subscription for SubjectSubscription<P, Sch, Cell>
where
  P: RcDerefMut<Target = Subscribers<O>>,
  Sch: Scheduler<Task<RemoveState<P>>>,
  Cell: SharedCell<SubscriptionState>,
{
  fn unsubscribe(self) {
    // Attempt to transition to Cancelled state.
    // We loop because compare_exchange can fail spuriously or due to state
    // change.
    loop {
      let current = self.state.get();
      match current {
        SubscriptionState::Cancelled => return, // Already cancelled
        SubscriptionState::Pending => {
          // If pending, try to cancel. If successful, the deferred task will
          // see Cancelled and abort.
          if self
            .state
            .compare_exchange(SubscriptionState::Pending, SubscriptionState::Cancelled)
            .is_ok()
          {
            return;
          }
        }
        SubscriptionState::Ready(id) => {
          // If ready, try to cancel. If successful, we must remove from list.
          if self
            .state
            .compare_exchange(SubscriptionState::Ready(id), SubscriptionState::Cancelled)
            .is_ok()
          {
            if let Some(mut guard) = self.observers.try_rc_deref_mut() {
              let _ob = guard.remove(id);
              return;
            }

            let task = Task::new(RemoveState { observers: self.observers, id }, |state| {
              let _ob = { state.observers.rc_deref_mut().remove(state.id) };
              TaskState::Finished
            });
            let _handle = self.scheduler.schedule(task, None);
            return;
          }
        }
      }
    }
  }

  fn is_closed(&self) -> bool { self.state.get() == SubscriptionState::Cancelled }
}
