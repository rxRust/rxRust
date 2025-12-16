use super::{
  subject_subscription::{RemoveState, SubjectSubscription, SubscriptionState},
  subscribers::Subscribers,
};
use crate::{
  context::{Context, MutArc, MutRc, RcDeref, RcDerefMut, SharedCell},
  observable::{CoreObservable, ObservableType},
  observer::{
    BoxedObserver, BoxedObserverMutRef, BoxedObserverMutRefSend, BoxedObserverSend,
    IntoBoxedObserver, Observer,
  },
  scheduler::{Scheduler, Task, TaskState},
};

// ============================================================================
// Type Aliases for Subject Pointer Types
// ============================================================================

/// Subject pointer type for value observers.
///
/// This alias simplifies the complex nested type
/// `C::Rc<Subscribers<C::BoxedObserver<...>>>`. It encapsulates the
/// reference-counted, mutable container of boxed observers.
///
/// # Type Parameters
///
/// - `'a`: Lifetime of the observers
/// - `C`: The context type
/// - `Item`: The item type emitted by the subject
/// - `Err`: The error type emitted by the subject
///
/// # Example
///
/// Instead of writing:
/// ```rust,ignore
/// Subject<C::Rc<Subscribers<C::BoxedObserver<'a, Item, Err>>>>
/// ```
///
/// You can write:
/// ```rust,ignore
/// // Most ergonomic in operator code where `Self: Observable`
/// Subject<SubjectPtr<'a, Self, Self::Item<'a>, Self::Err>>
/// ```
pub type SubjectPtr<'a, C, Item, Err> =
  <C as Context>::RcMut<Subscribers<<C as Context>::BoxedObserver<'a, Item, Err>>>;

/// Subject pointer type for mutable reference observers.
///
/// Similar to [`SubjectPtr`], but for subjects that broadcast mutable
/// references. This is used with `subject_mut_ref` and similar patterns.
///
/// # Type Parameters
///
/// - `'a`: Lifetime of the observers
/// - `C`: The context type (e.g., `Local`, `Shared`)
/// - `Item`: The item type (observers receive `&mut Item`)
/// - `Err`: The error type emitted by the subject
pub type SubjectPtrMutRef<'a, C, Item, Err> =
  <C as Context>::RcMut<Subscribers<<C as Context>::BoxedObserverMutRef<'a, Item, Err>>>;

/// Type alias for a `Subject` that operates in a local context (`Local`) and
/// broadcasts owned values (`Item`).
///
/// This simplifies the declaration of a common `Subject` variant.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items emitted by the subject (must be `Clone`).
/// - `Err`: The type of errors emitted by the subject.
pub type InnerSubject<'a, Item, Err> = Subject<MutRc<Subscribers<BoxedObserver<'a, Item, Err>>>>;

/// Type alias for a `Subject` that operates in a shared (thread-safe) context
/// (`Shared`) and broadcasts owned values (`Item`).
///
/// This simplifies the declaration of a common `Subject` variant.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items emitted by the subject (must be `Clone` and
///   `Send`).
/// - `Err`: The type of errors emitted by the subject (must be `Send`).
pub type InnerSubjectSend<'a, Item, Err> =
  Subject<MutArc<Subscribers<BoxedObserverSend<'a, Item, Err>>>>;

/// Type alias for a `Subject` that operates in a local context (`Local`) and
/// broadcasts mutable references (`&mut Item`).
///
/// This simplifies the declaration of a common `Subject` variant.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items whose mutable references are emitted by the
///   subject.
/// - `Err`: The type of errors emitted by the subject.
pub type InnerSubjectMutRef<'a, Item, Err> =
  Subject<MutRc<Subscribers<BoxedObserverMutRef<'a, Item, Err>>>>;

/// Type alias for a `Subject` that operates in a shared (thread-safe) context
/// (`Shared`) and broadcasts mutable references (`&mut Item`).
///
/// This simplifies the declaration of a common `Subject` variant.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items whose mutable references are emitted by the
///   subject (must be `Send`).
/// - `Err`: The type of errors emitted by the subject (must be `Send`).
pub type InnerSubjectMutRefSend<'a, Item, Err> =
  Subject<MutArc<Subscribers<BoxedObserverMutRefSend<'a, Item, Err>>>>;

/// Type alias for a `Local` subject that broadcasts owned values.
///
/// This type represents a `Subject` wrapped in a `Local` context,
/// making it convenient for local, non-`Send` scenarios.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items emitted by the subject (must be `Clone`).
/// - `Err`: The type of errors emitted by the subject.
#[cfg(feature = "scheduler")]
pub type LocalSubject<'a, Item, Err> = Local<InnerSubject<'a, Item, Err>>;

/// Type alias for a `Shared` subject that broadcasts owned values.
///
/// This type represents a `Subject` wrapped in a `Shared` context,
/// making it convenient for thread-safe, `Send` scenarios.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items emitted by the subject (must be `Clone` and
///   `Send`).
/// - `Err`: The type of errors emitted by the subject (must be `Send`).
#[cfg(feature = "scheduler")]
pub type SharedSubject<'a, Item, Err> = Shared<InnerSubjectSend<'a, Item, Err>>;

/// Type alias for a `Local` subject that broadcasts mutable references.
///
/// This type represents a `Subject` that emits mutable references, wrapped in
/// a `Local` context, suitable for local, non-`Send` scenarios where direct
/// modification of emitted values is desired.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items whose mutable references are emitted by the
///   subject.
/// - `Err`: The type of errors emitted by the subject.
#[cfg(feature = "scheduler")]
pub type LocalSubjectMutRef<'a, Item, Err> = Local<InnerSubjectMutRef<'a, Item, Err>>;

/// Type alias for a `Shared` subject that broadcasts mutable references.
///
/// This type represents a `Subject` that emits mutable references, wrapped in
/// a `Shared` context, suitable for thread-safe, `Send` scenarios where direct
/// modification of emitted values is desired across threads.
///
/// # Type Parameters
///
/// - `'a`: The lifetime of the observers.
/// - `Item`: The type of items whose mutable references are emitted by the
///   subject (must be `Send`).
/// - `Err`: The type of errors emitted by the subject (must be `Send`).
#[cfg(feature = "scheduler")]
pub type SharedSubjectMutRef<'a, Item, Err> = Shared<InnerSubjectMutRefSend<'a, Item, Err>>;

/// Subject: A hot observable that multicasts values to many observers.
///
/// The `Subject` struct acts as both an `Observer` and an `Observable`.
/// It maintains a list of subscribers and multicasts any value it receives to
/// all of them.
///
/// # Architecture
///
/// ## Pointer-Based Design
///
/// The Subject is now parameterized by a single smart pointer type `P`, which
/// points to `Subscribers<O>`. This design completely decouples Subject from
/// the `Context` trait:
/// - **Local**: Uses `Rc<RefCell<Subscribers<O>>>`
/// - **Shared**: Uses `Arc<Mutex<Subscribers<O>>>`
///
/// This design eliminates code duplication and makes Subject more pure and
/// reusable.
///
/// ## Type Parameters
///
/// - `P`: The smart pointer type (e.g., `Rc<RefCell<Subscribers<O>>>` or
///   `Arc<Mutex<Subscribers<O>>>`) Must implement `RcDerefMut` for unified
///   access to the inner `Subscribers`.
///
/// ## Observer Type Variants
///
/// The Subject supports two main observer patterns:
///
/// 1. **Value Observers**: `Box<dyn Observer<Item, Err>>`
///    - Requires `Item: Clone` for multicasting
///    - Broadcasts cloned values to all observers
///
/// 2. **Mutable Reference Observers**: `Box<dyn for<'a> Observer<&'a mut Item,
///    Err>>`
///    - Uses HRTB (Higher-Rank Trait Bounds)
///    - Allows sequential modification via re-borrowing
///    - No `Clone` requirement on `Item`
///
/// # Example
///
/// ```rust
/// use std::{cell::RefCell, convert::Infallible, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// // Value broadcasting
/// let subject = Local::subject::<i32, Infallible>();
/// let results = Rc::new(RefCell::new(vec![]));
/// let c_results = results.clone();
///
/// subject.clone().subscribe(move |v| {
///   c_results.borrow_mut().push(v);
/// });
///
/// subject.clone().next(1);
/// subject.clone().next(2);
/// assert_eq!(*results.borrow(), vec![1, 2]);
///
/// // Mutable reference broadcasting
/// let mut_subject = Local::subject_mut_ref::<i32, Infallible>();
/// mut_subject
///   .clone()
///   .subscribe(|v: &mut i32| *v += 1);
/// mut_subject
///   .clone()
///   .subscribe(|v: &mut i32| *v *= 2);
///
/// let mut value = 10;
/// mut_subject.clone().next(&mut value);
/// assert_eq!(value, 22); // (10 + 1) * 2
/// ```
///
/// # Safety and Limitations
///
/// ## Re-Entrancy Policy
///
/// - **Emissions (`next`/`error`/`complete`) are not re-entrant**. Calling
///   `next`/`error`/`complete` on the same `Subject` from within a callback of
///   that `Subject` will **panic**.
/// - **Subscription mutations are allowed** (`subscribe`/`unsubscribe`) inside
///   callbacks. They may be applied after the current emission finishes.
///
/// ### Subscription mutation semantics (simplified)
///
/// For `subscribe`/`unsubscribe`, `Subject` uses a single rule:
///
/// - If the internal subscribers container is not currently borrowed/locked,
///   the mutation is applied **synchronously**.
/// - If it *is* currently borrowed/locked (this happens during a synchronous
///   broadcast), the mutation is **deferred** by scheduling a task on the
///   subscribing context's scheduler.
///
/// This yields the following concrete behavior:
///
/// - `subscribe()` called from inside one of the subject’s callbacks:
///   - The new subscriber **will not** receive the *currently in-progress*
///     emission.
///   - It becomes active **after** the current emission finishes and the
///     scheduler runs the deferred add.
/// - `unsubscribe()` called from inside one of the subject’s callbacks:
///   - The unsubscribing observer may still receive the *currently in-progress*
///     emission (because it’s already being invoked).
///   - It will not receive subsequent emissions once the deferred removal runs.
/// - `unsubscribe()` on a subscription returned by a deferred `subscribe()`
///   (i.e. before activation) **cancels** the pending add, so the observer
///   never becomes active.
///
/// Ordering: within a single thread and a single scheduler, multiple deferred
/// subscription mutations are executed in the order they are scheduled by that
/// scheduler.
///
/// This keeps ordering predictable and prevents accidental infinite feedback
/// loops. If you intentionally need feedback loops, insert an explicit async
/// boundary (e.g. `delay(Duration::from_millis(0))`).
///
/// ### Example: re-entrant emission (will panic)
///
/// ```rust,no_run
/// use std::convert::Infallible;
///
/// use rxrust::prelude::*;
///
/// let subject = Local::subject::<i32, Infallible>();
/// let s_clone = subject.clone();
///
/// // WRONG: This will panic!
/// subject.clone().subscribe(move |_| {
///   s_clone.clone().next(1); // Panic: re-entrant emission
/// });
/// ```
///
/// ### Example: explicit async boundary (works)
///
/// ```rust,no_run
/// use std::{convert::Infallible, time::Duration};
///
/// use rxrust::prelude::*;
///
/// let subject = Local::subject::<i32, Infallible>();
/// let s_clone = subject.clone();
///
/// subject.clone().subscribe(move |_| {
///   s_clone
///     .clone()
///     .delay(Duration::from_millis(0))
///     .subscribe(|_| println!("Runs on a later tick"));
/// });
/// ```
pub struct Subject<P> {
  pub observers: P,
}

// ============================================================================
// Factory Methods
// ============================================================================

#[cfg(feature = "scheduler")]
use crate::context::{Local, Shared};
#[cfg(feature = "scheduler")]
use crate::scheduler::{LocalScheduler, SharedScheduler};

#[cfg(feature = "scheduler")]
impl<'a, Item, Err> Subject<MutRc<Subscribers<BoxedObserver<'a, Item, Err>>>> {
  /// Create a new local subject for value broadcasting.
  pub fn local() -> Local<Self> { Local { inner: Self::default(), scheduler: LocalScheduler } }
}

#[cfg(feature = "scheduler")]
impl<'a, Item, Err> Subject<MutArc<Subscribers<BoxedObserverSend<'a, Item, Err>>>> {
  /// Create a new shared subject for value broadcasting.
  pub fn shared() -> Shared<Self> { Shared { inner: Self::default(), scheduler: SharedScheduler } }
}

#[cfg(feature = "scheduler")]
impl<'a, Item: 'a, Err> Subject<MutRc<Subscribers<BoxedObserverMutRef<'a, Item, Err>>>> {
  /// Create a new local subject for mutable reference broadcasting.
  pub fn local_mut_ref() -> Local<Self> {
    Local { inner: Self::default(), scheduler: LocalScheduler }
  }
}

#[cfg(feature = "scheduler")]
impl<'a, Item: 'a, Err> Subject<MutArc<Subscribers<BoxedObserverMutRefSend<'a, Item, Err>>>> {
  /// Create a new shared subject for mutable reference broadcasting.
  pub fn shared_mut_ref() -> Shared<Self> {
    Shared { inner: Self::default(), scheduler: SharedScheduler }
  }
}

impl<P, O> Subject<P>
where
  P: RcDeref<Target = Subscribers<O>>,
{
  /// Get the number of current subscribers.
  pub fn subscriber_count(&self) -> usize { self.observers.rc_deref().inner.len() }

  /// Check if there are no subscribers.
  pub fn is_empty(&self) -> bool { self.observers.rc_deref().inner.is_empty() }
}

// ============================================================================
// Standard Traits
// ============================================================================

impl<P: Clone> Clone for Subject<P> {
  fn clone(&self) -> Self { Self { observers: self.observers.clone() } }
}

impl<P> Default for Subject<P>
where
  P: RcDeref<Target: Default> + From<P::Target>,
{
  fn default() -> Self { Self { observers: P::from(P::Target::default()) } }
}

// ============================================================================
// Observer Implementation Macro
// ============================================================================

/// Generates Observer implementations for Subject.
/// - `value` arm: For value broadcasting (requires Item: Clone)
/// - `mut_ref` arm: For mutable reference broadcasting
macro_rules! impl_observer_for_subject {
  (value, $ptr:ident, $obs:ident) => {
    #[allow(coherence_leak_check)]
    impl<'a, Item, Err> Observer<Item, Err> for Subject<$ptr<Subscribers<$obs<'a, Item, Err>>>>
    where
      Item: Clone,
      Err: Clone,
    {
      fn next(&mut self, value: Item) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_value(value);
          return;
        }
        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn error(self, err: Err) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_error(err);
          return;
        }
        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn complete(self) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_complete();
          return;
        }
        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn is_closed(&self) -> bool { self.observers.rc_deref().inner.is_empty() }
    }
  };

  (mut_ref, $ptr:ident, $obs:ident) => {
    #[allow(coherence_leak_check)]
    impl<'a, Item, Err> Observer<&mut Item, Err> for Subject<$ptr<Subscribers<$obs<'a, Item, Err>>>>
    where
      Err: Clone,
    {
      fn next(&mut self, value: &mut Item) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_mut_ref(value);
          return;
        }

        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn error(self, err: Err) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_error(err);
          return;
        }
        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn complete(self) {
        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          guard.broadcast_complete();
          return;
        }
        panic!(
          "re-entrant Subject emissions are not supported (next/error/complete). Use an explicit \
           async boundary (e.g. delay(0)) if you need feedback loops."
        );
      }

      fn is_closed(&self) -> bool { self.observers.rc_deref().inner.is_empty() }
    }
  };
}

// Generate all Observer implementations
impl_observer_for_subject!(value, MutRc, BoxedObserver);
impl_observer_for_subject!(value, MutArc, BoxedObserverSend);
impl_observer_for_subject!(mut_ref, MutRc, BoxedObserverMutRef);
impl_observer_for_subject!(mut_ref, MutArc, BoxedObserverMutRefSend);

// ============================================================================
// CoreObservable Implementation Macro
// ============================================================================

/// Generates ObservableType and CoreObservable implementations for Subject
macro_rules! impl_core_observable_for_subject {
  ($ptr:ident, $obs:ident, $item_type:ty) => {
    #[allow(coherence_leak_check)]
    impl<'a, Item, Err> ObservableType for Subject<$ptr<Subscribers<$obs<'a, Item, Err>>>> {
      type Item<'m>
        = $item_type
      where
        Self: 'm;
      type Err = Err;
    }

    #[allow(coherence_leak_check)]
    impl<'a, Item, Err, C> CoreObservable<C> for Subject<$ptr<Subscribers<$obs<'a, Item, Err>>>>
    where
      C: Context,
      C::Inner: IntoBoxedObserver<$obs<'a, Item, Err>>,
      C::Scheduler: Scheduler<
        Task<
          AddState<
            $ptr<Subscribers<$obs<'a, Item, Err>>>,
            $obs<'a, Item, Err>,
            C::RcCell<SubscriptionState>,
          >,
        >,
      >,
      C::Scheduler: Scheduler<Task<RemoveState<$ptr<Subscribers<$obs<'a, Item, Err>>>>>>,
    {
      type Unsub = SubjectSubscription<
        $ptr<Subscribers<$obs<'a, Item, Err>>>,
        C::Scheduler,
        C::RcCell<SubscriptionState>,
      >;

      fn subscribe(self, observer: C) -> Self::Unsub {
        let scheduler = observer.scheduler().clone();
        let boxed = observer.into_inner().into_boxed();
        let observers_ptr = self.observers.clone();
        let state = C::RcCell::<SubscriptionState>::from(SubscriptionState::Pending);

        if let Some(mut guard) = self.observers.try_rc_deref_mut() {
          let id = guard.add(boxed);
          state.set(SubscriptionState::Ready(id));
          return SubjectSubscription::new(observers_ptr, state, scheduler);
        }

        let sub = SubjectSubscription::new(observers_ptr, state.clone(), scheduler.clone());
        let observers = self.observers;

        let task = Task::new(AddState { observers, boxed: Some(boxed), state }, |task_state| {
          let current_state = task_state.state.get();
          // If cancelled, do nothing.
          if current_state == SubscriptionState::Cancelled {
            return TaskState::Finished;
          }

          let boxed = task_state
            .boxed
            .take()
            .expect("add executed twice");

          // Insert and get ID.
          let mut guard = task_state.observers.rc_deref_mut();
          let id = guard.add(boxed);

          // Attempt to transition to Ready(id).
          // If state changed to Cancelled during insertion, we must rollback.
          if task_state
            .state
            .compare_exchange(SubscriptionState::Pending, SubscriptionState::Ready(id))
            .is_err()
          {
            // Must have been cancelled.
            let _ = guard.remove(id);
          }

          TaskState::Finished
        });
        let _handle = scheduler.schedule(task, None);
        sub
      }
    }
  };
}

struct AddState<P, Ob, Cell> {
  observers: P,
  boxed: Option<Ob>,
  state: Cell,
}

// Generate all CoreObservable implementations
impl_core_observable_for_subject!(MutRc, BoxedObserver, Item);
impl_core_observable_for_subject!(MutArc, BoxedObserverSend, Item);
impl_core_observable_for_subject!(MutRc, BoxedObserverMutRef, &'m mut Item);
impl_core_observable_for_subject!(MutArc, BoxedObserverMutRefSend, &'m mut Item);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use std::{
    cell::RefCell,
    convert::Infallible,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  use super::*;
  use crate::{observable::connectable::Connectable, prelude::*};

  #[rxrust_macro::test]
  fn test_local_subject() {
    let subject = Local::subject();
    let results = Rc::new(RefCell::new(vec![]));
    let c_results = results.clone();

    subject.clone().subscribe(move |v| {
      c_results.borrow_mut().push(v);
    });

    subject.clone().next(1);
    subject.clone().next(2);

    assert_eq!(*results.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_local_subject_multiple_subscribers() {
    let subject = Local::subject();
    let results1 = Rc::new(RefCell::new(vec![]));
    let results2 = Rc::new(RefCell::new(vec![]));

    let c1 = results1.clone();
    subject
      .clone()
      .subscribe(move |v| c1.borrow_mut().push(v));

    subject.clone().next(1);

    let c2 = results2.clone();
    subject
      .clone()
      .subscribe(move |v| c2.borrow_mut().push(v));

    subject.clone().next(2);

    assert_eq!(*results1.borrow(), vec![1, 2]);
    assert_eq!(*results2.borrow(), vec![2]);
  }

  #[rxrust_macro::test]
  fn test_shared_subject() {
    let subject = Shared::subject();
    let results = Arc::new(Mutex::new(vec![]));
    let c_results = results.clone();

    subject.clone().subscribe(move |v| {
      c_results.lock().unwrap().push(v);
    });

    let mut s = subject.clone();
    s.next(1);
    s.next(2);

    assert_eq!(*results.lock().unwrap(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_unsubscribe() {
    use crate::subscription::Subscription;
    let subject = Local::subject();
    let results = Rc::new(RefCell::new(vec![]));
    let c_results = results.clone();

    let sub = subject.clone().subscribe(move |v| {
      c_results.borrow_mut().push(v);
    });

    subject.clone().next(1);
    sub.unsubscribe();
    subject.clone().next(2);

    assert_eq!(*results.borrow(), vec![1]);
  }

  #[cfg(all(feature = "scheduler", not(target_arch = "wasm32")))]
  #[rxrust_macro::test(local)]
  async fn test_subject_unsubscribe_inside_next_is_deferred() {
    use crate::subscription::Subscription;

    let subject = Local::subject::<i32, Infallible>();

    let results_primary = Rc::new(RefCell::new(vec![]));
    let results_secondary = Rc::new(RefCell::new(vec![]));

    // Secondary observer we will unsubscribe from inside the primary callback.
    let sub_secondary = subject.clone().subscribe({
      let results_secondary = results_secondary.clone();
      move |v| results_secondary.borrow_mut().push(v)
    });

    let sub_secondary_cell = Rc::new(RefCell::new(Some(sub_secondary)));

    // Primary observer unsubscribes from inside the callback.
    subject.clone().subscribe({
      let _subject = subject.clone();
      let results_primary = results_primary.clone();
      let sub_secondary_cell = sub_secondary_cell.clone();
      move |v| {
        results_primary.borrow_mut().push(v);
        if v == 1
          && let Some(sub) = sub_secondary_cell.borrow_mut().take()
        {
          sub.unsubscribe();
        }
      }
    });

    subject.clone().next(1);

    // Let deferred tasks run.
    crate::scheduler::tokio::task::yield_now().await;
    crate::scheduler::tokio::task::yield_now().await;

    assert_eq!(*results_primary.borrow(), vec![1]);
    // Secondary should have seen the first emission.
    assert_eq!(*results_secondary.borrow(), vec![1]);

    // Next emission should not reach secondary (unsubscribe applied).
    subject.clone().next(2);
    crate::scheduler::tokio::task::yield_now().await;

    assert_eq!(*results_primary.borrow(), vec![1, 2]);
    assert_eq!(*results_secondary.borrow(), vec![1]);
  }

  #[cfg(all(feature = "scheduler", not(target_arch = "wasm32")))]
  #[rxrust_macro::test(local)]
  async fn test_subject_subscribe_inside_next_is_deferred() {
    use crate::subscription::Subscription;

    let subject = Local::subject::<i32, Infallible>();

    let primary = Rc::new(RefCell::new(vec![]));
    let secondary = Rc::new(RefCell::new(vec![]));

    // Keep the secondary subscription alive after it is created inside the
    // callback.
    let secondary_sub: Rc<RefCell<Option<_>>> = Rc::new(RefCell::new(None));

    subject.clone().subscribe({
      let subject = subject.clone();
      let primary = primary.clone();
      let secondary = secondary.clone();
      let secondary_sub = secondary_sub.clone();
      move |v| {
        primary.borrow_mut().push(v);
        if v == 1 {
          let sub = subject.clone().subscribe({
            let secondary = secondary.clone();
            move |v| secondary.borrow_mut().push(v)
          });
          *secondary_sub.borrow_mut() = Some(sub);
        }
      }
    });

    // Emitting 1 triggers a subscribe inside the callback.
    subject.clone().next(1);

    // Let deferred add run.
    crate::scheduler::tokio::task::yield_now().await;
    crate::scheduler::tokio::task::yield_now().await;

    // Secondary should NOT have seen the in-progress emission.
    assert_eq!(*primary.borrow(), vec![1]);
    assert_eq!(*secondary.borrow(), Vec::<i32>::new());

    // Now the secondary subscription should be active.
    subject.clone().next(2);
    crate::scheduler::tokio::task::yield_now().await;

    assert_eq!(*primary.borrow(), vec![1, 2]);
    assert_eq!(*secondary.borrow(), vec![2]);

    // Clean up.
    if let Some(sub) = secondary_sub.borrow_mut().take() {
      sub.unsubscribe();
    }
  }

  #[cfg(all(feature = "scheduler", not(target_arch = "wasm32")))]
  #[rxrust_macro::test(local)]
  async fn test_subject_unsubscribe_cancels_pending_subscribe() {
    let subject = Local::subject::<i32, Infallible>();

    let secondary = Rc::new(RefCell::new(vec![]));

    subject.clone().subscribe({
      let subject = subject.clone();
      let secondary = secondary.clone();
      move |v| {
        if v == 1 {
          // This subscribe happens during broadcast, so it's deferred.
          // Immediately unsubscribing should cancel the pending add.
          let sub = subject.clone().subscribe({
            let secondary = secondary.clone();
            move |v| secondary.borrow_mut().push(v)
          });
          sub.unsubscribe();
        }
      }
    });

    subject.clone().next(1);

    // Let deferred add/removal tasks run.
    crate::scheduler::tokio::task::yield_now().await;
    crate::scheduler::tokio::task::yield_now().await;

    // If cancellation worked, secondary never becomes active.
    subject.clone().next(2);
    crate::scheduler::tokio::task::yield_now().await;

    assert!(secondary.borrow().is_empty());
  }

  // These tests rely on `std::panic::catch_unwind` to observe panics from
  // re-entrant Subject emissions. On `wasm32` targets panics are typically
  // handled as aborts / JS `RuntimeError` and cannot be caught by Rust's
  // unwind machinery, so the tests would always fail under `wasm-bindgen`.
  // If you need to support catching panics on wasm, enable `panic = "unwind"`
  // and wasm exception/unwind support in the toolchain — however that is
  // non-trivial and not suitable for general CI. Therefore skip on wasm.
  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  fn test_subject_reentrant_next_panics() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let subject = Local::subject::<i32, Infallible>();
    subject.clone().subscribe({
      let subject = subject.clone();
      move |_| {
        subject.clone().next(2);
      }
    });

    let result = catch_unwind(AssertUnwindSafe(|| {
      subject.clone().next(1);
    }));
    assert!(result.is_err());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  fn test_subject_reentrant_complete_panics() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let subject = Local::subject::<i32, Infallible>();
    subject.clone().subscribe({
      let subject = subject.clone();
      move |_| {
        subject.clone().complete();
      }
    });

    let result = catch_unwind(AssertUnwindSafe(|| {
      subject.clone().next(1);
    }));
    assert!(result.is_err());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  fn test_subject_reentrant_error_panics() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct ReentrantError<S> {
      subject: S,
    }

    impl<S> Observer<i32, ()> for ReentrantError<S>
    where
      S: Clone + Observer<i32, ()>,
    {
      fn next(&mut self, _value: i32) { self.subject.clone().error(()); }

      fn error(self, _err: ()) {}

      fn complete(self) {}

      fn is_closed(&self) -> bool { false }
    }

    let subject = Local::subject::<i32, ()>();
    subject
      .clone()
      .subscribe_with(ReentrantError { subject: subject.clone() });

    let result = catch_unwind(AssertUnwindSafe(|| {
      subject.clone().next(1);
    }));
    assert!(result.is_err());
  }

  #[rxrust_macro::test]
  fn test_mut_ref_subject_emit() {
    let subject = Local::subject_mut_ref();
    let mut producer = subject.clone();

    // Expected pattern - modify value sequentially
    subject.clone().subscribe(|v: &mut i32| *v += 1);
    subject.subscribe(|v: &mut i32| *v *= 2);

    let mut value = 10;
    producer.next(&mut value);

    assert_eq!(value, 22);
  }

  #[rxrust_macro::test]
  fn test_behavior_subject() {
    // Factory might need extension for behavior_subject or use constructor directly
    let mut bs = Shared::behavior_subject::<i32, Infallible>(0);

    // Test basic functionality - let's verify structure
    bs.next(1);

    let results = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let results_clone = results.clone();

    bs.clone().subscribe(move |v| {
      results_clone.lock().unwrap().push(v);
    });

    // Should immediately emit current value (1)
    assert_eq!(*results.lock().unwrap(), vec![1]);

    // Further emissions should continue to work
    bs.next(2);
    assert_eq!(*results.lock().unwrap(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_scoped_subject() {
    let x = 100;
    let subject = Local::subject();
    let mut producer = subject.clone();

    // Captures 'x' which is local
    subject.clone().subscribe(move |v| {
      assert_eq!(v + x, 110);
    });

    producer.next(10);
  }

  #[rxrust_macro::test]
  fn test_behavior_subject_multiple_subscribers() {
    let mut bs = Shared::behavior_subject::<i32, Infallible>(0);

    let results1 = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let results2 = std::sync::Arc::new(std::sync::Mutex::new(vec![]));

    // Clone the results for each subscriber
    let r1 = results1.clone();
    let r2 = results2.clone();

    bs.clone().subscribe(move |v| {
      r1.lock().unwrap().push(v);
    });

    bs.clone().subscribe(move |v| {
      r2.lock().unwrap().push(v);
    });

    // Both subscribers should immediately receive current value (0)
    assert_eq!(*results1.lock().unwrap(), vec![0]);
    assert_eq!(*results2.lock().unwrap(), vec![0]);

    // First emission - both subscribers should receive
    bs.next(1);
    assert_eq!(*results1.lock().unwrap(), vec![0, 1]);
    assert_eq!(*results2.lock().unwrap(), vec![0, 1]);

    // Second emission - both subscribers should receive
    bs.next(2);
    assert_eq!(*results1.lock().unwrap(), vec![0, 1, 2]);
    assert_eq!(*results2.lock().unwrap(), vec![0, 1, 2]);
  }

  #[rxrust_macro::test]
  fn test_subject_with_publish_integration() {
    use crate::observable::Observable;

    // Test that Subject works as intended with publish()
    let source = Local::of(1).merge(Local::of(2));
    let connectable = source.publish();

    let results = Rc::new(RefCell::new(vec![]));
    let results_clone = results.clone();

    // Create multiple observables via fork
    let obs1 = connectable.fork();
    let obs2 = connectable.fork();

    // Subscribe both observers
    obs1.subscribe(move |v| {
      results_clone
        .borrow_mut()
        .push(format!("obs1: {}", v));
    });

    let results_clone2 = results.clone();
    obs2.subscribe(move |v| {
      results_clone2
        .borrow_mut()
        .push(format!("obs2: {}", v));
    });

    // Connect to start multicasting
    connectable.connect();

    // Both observers should receive the same values
    let received = results.borrow();
    assert!(received.contains(&"obs1: 1".to_string()));
    assert!(received.contains(&"obs1: 2".to_string()));
    assert!(received.contains(&"obs2: 1".to_string()));
    assert!(received.contains(&"obs2: 2".to_string()));
  }

  #[rxrust_macro::test]
  fn test_subject_with_publish_mut_ref_integration() {
    use crate::observable::Observable;

    // Test mutable reference broadcasting with publish_mut_ref()
    let mut source = Local::subject_mut_ref();
    let connectable = source.clone().publish_mut_ref();

    let results = Rc::new(RefCell::new(vec![]));
    let results_clone = results.clone();

    // Create observables via fork
    let obs1 = connectable.fork();
    let obs2 = connectable.fork();

    // First observer increments the value
    obs1.subscribe(move |v: &mut i32| {
      *v += 1;
      results_clone
        .borrow_mut()
        .push(format!("obs1: {}", v));
    });

    let results_clone2 = results.clone();
    // Second observer multiplies the value (after increment)
    obs2.subscribe(move |v: &mut i32| {
      *v *= 2;
      results_clone2
        .borrow_mut()
        .push(format!("obs2: {}", v));
    });

    // Connect to start multicasting
    let _connection = connectable.connect();

    // Emit initial value
    let mut test_value = 10;
    source.next(&mut test_value);

    // Check the sequence of modifications
    let received = results.borrow();
    // The value should be modified sequentially: 10 -> 11 -> 22
    assert!(received.contains(&"obs1: 11".to_string()));
    assert!(received.contains(&"obs2: 22".to_string()));
  }

  #[rxrust_macro::test]
  fn test_subject_multicast_with_different_subjects() {
    use crate::observable::Observable;

    // Test multicast with custom subject
    let source = Local::of(42);
    let custom_subject = Local::subject();
    let connectable = source.multicast(custom_subject.into_inner());

    let results = Rc::new(RefCell::new(vec![]));
    let results_clone = results.clone();

    let obs = connectable.fork();
    obs.subscribe(move |v| {
      results_clone.borrow_mut().push(v);
    });

    connectable.connect();

    assert_eq!(*results.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_subject_multicast_mut_ref_with_different_subjects() {
    use crate::observable::Observable;

    // Test multicast_mut_ref with custom subject
    let mut source = Local::subject_mut_ref();
    let custom_subject = Local::subject_mut_ref();
    let connectable = source
      .clone()
      .multicast_mut_ref(custom_subject.into_inner());

    let results = Rc::new(RefCell::new(vec![]));
    let results_clone = results.clone();

    let obs1 = connectable.fork();
    let obs2 = connectable.fork();

    // Sequential modifications
    obs1.subscribe(move |v: &mut i32| {
      *v += 100;
      results_clone
        .borrow_mut()
        .push(format!("obs1: {}", v));
    });

    let results_clone2 = results.clone();
    obs2.subscribe(move |v: &mut i32| {
      *v *= 3;
      results_clone2
        .borrow_mut()
        .push(format!("obs2: {}", v));
    });

    let _connection = connectable.connect();

    let mut test_value = 5;
    source.next(&mut test_value);

    let received = results.borrow();
    // Value should be modified: 5 -> 105 -> 315
    assert!(received.contains(&"obs1: 105".to_string()));
    assert!(received.contains(&"obs2: 315".to_string()));
  }

  #[rxrust_macro::test]
  fn test_subject_early_vs_late_subscription() {
    use crate::observable::Observable;

    // Test that early subscribers receive values, late subscribers don't receive
    // past values Use a simple subject instead of ConnectableObservable to
    // avoid Clone issues
    let subject = Local::subject();

    let early_results = Rc::new(RefCell::new(vec![]));
    let early_results_clone = early_results.clone();

    // Early subscriber
    subject.clone().subscribe(move |v| {
      early_results_clone.borrow_mut().push(v);
    });

    // Emit first value - early subscriber gets it
    subject.clone().next(1);

    // Late subscriber (after first emission)
    let late_results = Rc::new(RefCell::new(vec![]));
    let late_results_clone = late_results.clone();
    subject.clone().subscribe(move |v| {
      late_results_clone.borrow_mut().push(v);
    });

    // Emit second value - both subscribers get it
    subject.clone().next(2);

    // Early subscriber should receive both values
    assert_eq!(*early_results.borrow(), vec![1, 2]);
    // Late subscriber should only receive values from when they subscribed
    assert_eq!(*late_results.borrow(), vec![2]);
  }
}
