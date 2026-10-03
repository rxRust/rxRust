//! A shared cancellation handle for one subscription, including synchronous
//! setup.
use super::Subscription;
use crate::rc::RcDerefMut;

/// Installation state for one upstream subscription.
pub enum State<U> {
  Pending,
  Assigned(U),
  Cancelled,
}

/// Coordinates installation and cancellation without owning observer lifecycle.
/// Dropping a handle does not cancel. Cleanup always runs outside the state
/// lock.
pub struct SingleAssignment<P>(P);

impl<P: Clone> Clone for SingleAssignment<P> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<P, U> Default for SingleAssignment<P>
where
  P: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  fn default() -> Self { Self::new() }
}

impl<P, U> SingleAssignment<P>
where
  P: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  pub fn new() -> Self { Self(State::Pending.into()) }

  /// Installs the upstream once. A cancelled handle immediately cancels any
  /// late installation; installing twice on an active handle is a logic error.
  pub fn set(&self, upstream: U) {
    let mut state = self.0.rc_deref_mut();
    match &*state {
      State::Pending => *state = State::Assigned(upstream),
      State::Assigned(_) => {
        drop(state);
        panic!("subscription already assigned");
      }
      State::Cancelled => {
        drop(state);
        upstream.unsubscribe();
      }
    }
  }
}

impl<P, U> Subscription for SingleAssignment<P>
where
  P: RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  fn unsubscribe(self) {
    let previous = std::mem::replace(&mut *self.0.rc_deref_mut(), State::Cancelled);
    if let State::Assigned(upstream) = previous {
      upstream.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool {
    match &*self.0.rc_deref() {
      State::Pending => false,
      State::Assigned(upstream) => upstream.is_closed(),
      State::Cancelled => true,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::*;
  use crate::{rc::MutRc, subscription::ClosureSubscription};

  #[rxrust_macro::test]
  fn late_installation_and_repeated_cancellation() {
    let calls = Cell::new(0);
    let handle = SingleAssignment::<MutRc<_>>::new();
    handle.clone().unsubscribe();
    for _ in 0..2 {
      handle.set(ClosureSubscription(|| calls.set(calls.get() + 1)));
    }
    handle.clone().unsubscribe();
    assert!(handle.is_closed());
    assert_eq!(calls.get(), 2);
  }

  #[rxrust_macro::test]
  fn cleanup_is_outside_lock_and_releases_captures() {
    let handle = SingleAssignment::<MutRc<State<crate::subscription::BoxedSubscription>>>::new();
    let other = handle.clone();
    let value = std::rc::Rc::new(());
    let weak = std::rc::Rc::downgrade(&value);
    use crate::subscription::IntoBoxedSubscription;
    handle.set(
      ClosureSubscription(move || {
        assert!(other.is_closed());
        other.unsubscribe();
        drop(value);
      })
      .into_boxed(),
    );
    handle.unsubscribe();
    assert!(weak.upgrade().is_none());
  }

  #[rxrust_macro::test]
  fn ordinary_drop_does_not_cancel() {
    let calls = Cell::new(0);
    let handle = SingleAssignment::<MutRc<_>>::new();
    handle.set(ClosureSubscription(|| calls.set(calls.get() + 1)));
    drop(handle);
    assert_eq!(calls.get(), 0);
  }

  #[rxrust_macro::test]
  #[should_panic(expected = "subscription already assigned")]
  fn duplicate_installation_fails() {
    let handle = SingleAssignment::<MutRc<_>>::new();
    handle.set(());
    handle.set(());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn installation_races_cancellation() {
    use std::sync::{
      Arc, Barrier,
      atomic::{AtomicUsize, Ordering},
    };
    for _ in 0..64 {
      let calls = Arc::new(AtomicUsize::new(0));
      let handle = SingleAssignment::<crate::rc::MutArc<_>>::new();
      let other = handle.clone();
      let barrier = Arc::new(Barrier::new(2));
      let ready = barrier.clone();
      let count = calls.clone();
      let thread = std::thread::spawn(move || {
        ready.wait();
        other.set(ClosureSubscription(move || {
          count.fetch_add(1, Ordering::SeqCst);
        }));
      });
      barrier.wait();
      handle.unsubscribe();
      thread.join().unwrap();
      assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
  }
}
