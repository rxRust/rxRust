//! A single-assignment channel coordinating upstream setup and cancellation.
use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use smallvec::SmallVec;

use super::Subscription;
use crate::rc::{RcDeref, RcDerefMut};

/// Installation state for one upstream subscription.
#[derive(Clone)]
pub enum State<U> {
  /// Not installed yet; holds the tasks waiting for installation.
  Pending(SmallVec<[Waker; 1]>),
  Assigned(U),
  /// Cancelled; a late installation is immediately unsubscribed.
  Cancelled,
}

/// The subscription end of a single-assignment channel.
/// Dropping a handle does not cancel. Cleanup always runs outside the state
/// lock.
///
/// Cloning is available only when the stored subscription is cloneable. Clones
/// share the same installation and cancellation state; they do not clone the
/// upstream value.
///
/// ```compile_fail
/// use rxrust::{rc::MutRc, subscription::{SingleAssignment, Subscription}};
///
/// struct NotClone;
/// impl Subscription for NotClone {
///   fn unsubscribe(self) {}
///   fn is_closed(&self) -> bool { false }
/// }
/// let (install, [handle]) = SingleAssignment::<MutRc<_>>::channel();
/// install(NotClone);
/// let _ = handle.clone();
/// ```
pub struct SingleAssignment<P>(P);

impl<P> Clone for SingleAssignment<P>
where
  P: RcDeref<Target: Clone>,
{
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

// Callers receive only a one-use function; the concrete installer stays
// private.
struct Installer<P>(P);

impl<P, U> SingleAssignment<P>
where
  P: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  /// Creates a one-use installation function and a fixed number of handles.
  /// All handles share one installation and cancellation state. The array
  /// pattern at the call site determines how many handles are needed.
  ///
  /// Cancellation before installation is remembered: installing the upstream
  /// afterwards immediately cancels it. Construction does not require a
  /// cloneable upstream; cloning an existing handle does.
  ///
  /// Every handle has the same capabilities. If multiple handles are awaited,
  /// the installed upstream must support multiple waiters.
  ///
  /// Until installation or cancellation, awaiting the subscription stays
  /// pending. Dropping the installer does neither; the caller must arrange
  /// installation or cancellation to wake waiting tasks.
  ///
  /// ```
  /// use rxrust::{
  ///   rc::MutRc,
  ///   subscription::{SingleAssignment, Subscription},
  /// };
  ///
  /// let (install, [observer_handle, subscription]) = SingleAssignment::<MutRc<_>>::channel();
  /// observer_handle.unsubscribe();
  /// assert!(subscription.is_closed());
  /// // Cancellation is remembered before the upstream handle arrives.
  /// install(());
  /// ```
  ///
  /// The installer is consumed by its first call:
  ///
  /// ```compile_fail
  /// use rxrust::{rc::MutRc, subscription::SingleAssignment};
  /// let (install, [_subscription]) = SingleAssignment::<MutRc<_>>::channel();
  /// install(());
  /// install(());
  /// ```
  ///
  /// The installer cannot be cloned to obtain another installation:
  ///
  /// ```compile_fail
  /// use rxrust::{rc::MutRc, subscription::SingleAssignment};
  /// let (install, [_subscription]) = SingleAssignment::<MutRc<_>>::channel();
  /// let duplicate = install.clone();
  /// install(());
  /// duplicate(());
  /// ```
  pub fn channel<const N: usize>() -> (impl FnOnce(U), [Self; N]) {
    let state = P::from(State::Pending(SmallVec::new()));
    let handles = std::array::from_fn(|_| Self(state.clone()));
    let installer = Installer(state);
    (move |upstream| installer.set(upstream), handles)
  }
}

impl<P, U> Installer<P>
where
  P: RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  fn set(self, upstream: U) {
    let mut state = self.0.rc_deref_mut();
    match &mut *state {
      State::Pending(wakers) => {
        let wakers = std::mem::take(wakers);
        *state = State::Assigned(upstream);
        drop(state);
        for waker in wakers {
          waker.wake();
        }
      }
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
    match previous {
      State::Assigned(upstream) => upstream.unsubscribe(),
      State::Pending(wakers) => {
        for waker in wakers {
          waker.wake();
        }
      }
      State::Cancelled => {}
    }
  }

  fn is_closed(&self) -> bool {
    match &*self.0.rc_deref() {
      State::Pending(_) => false,
      State::Assigned(upstream) => upstream.is_closed(),
      State::Cancelled => true,
    }
  }
}

/// Resolves when the upstream future resolves or the handle is cancelled.
/// Installation or cancellation wakes all tasks waiting for installation.
/// After installation, polling and notification are delegated to `U`, including
/// its support for multiple waiters and polling after completion.
/// `U` must wake its waker when unsubscribed, as `TaskHandle` does.
/// `U::poll` runs while the state lock is held and must not re-enter
/// the same `SingleAssignment` through any of its clones.
impl<P, U> Future for SingleAssignment<P>
where
  P: RcDerefMut<Target = State<U>>,
  U: Future<Output = ()> + Unpin,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    match &mut *self.0.rc_deref_mut() {
      State::Pending(wakers) => {
        if !wakers
          .iter()
          .any(|waker| waker.will_wake(cx.waker()))
        {
          wakers.push(cx.waker().clone());
        }
        Poll::Pending
      }
      State::Assigned(upstream) => Pin::new(upstream).poll(cx),
      State::Cancelled => Poll::Ready(()),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use futures::{FutureExt, executor::LocalPool, task::LocalSpawnExt};

  use super::*;
  use crate::{rc::MutRc, subscription::ClosureSubscription};

  #[derive(Clone)]
  struct Done;

  impl Future for Done {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> { Poll::Ready(()) }
  }

  impl Subscription for Done {
    fn unsubscribe(self) {}
    fn is_closed(&self) -> bool { true }
  }

  fn pending_waiters<F: Future<Output = ()> + Clone + 'static>(
    handle: F,
  ) -> (LocalPool, Rc<Cell<usize>>) {
    let mut pool = LocalPool::new();
    let completed = Rc::new(Cell::new(0));
    for waiter in [handle.clone(), handle] {
      let completed = completed.clone();
      pool
        .spawner()
        .spawn_local(async move {
          waiter.await;
          completed.set(completed.get() + 1);
        })
        .unwrap();
    }
    // Run each independent task until its first Pending, without busy polling.
    pool.run_until_stalled();
    assert_eq!(completed.get(), 0);
    (pool, completed)
  }

  #[rxrust_macro::test]
  fn installation_wakes_all_pending_waiters() {
    let (install, [handle]) = SingleAssignment::<MutRc<_>>::channel();
    let (mut pool, completed) = pending_waiters(handle);
    install(Done);
    pool.run_until_stalled();
    assert_eq!(completed.get(), 2);
  }

  #[rxrust_macro::test]
  fn cancellation_wakes_all_pending_waiters() {
    let (_install, [cancel, handle]) = SingleAssignment::<MutRc<State<Done>>>::channel();
    let (mut pool, completed) = pending_waiters(handle);
    cancel.unsubscribe();
    pool.run_until_stalled();
    assert_eq!(completed.get(), 2);
  }

  #[rxrust_macro::test]
  fn assigned_waiters_follow_upstream_completion() {
    let (install, [handle]) = SingleAssignment::<MutRc<_>>::channel();
    let task = crate::scheduler::TaskHandle::new();
    install(task.clone());
    let (mut pool, completed) = pending_waiters(handle);
    task.close();
    pool.run_until_stalled();
    assert_eq!(completed.get(), 2);
  }

  #[rxrust_macro::test]
  fn cancellation_before_await_is_ready() {
    let (_install, [cancel, handle]) = SingleAssignment::<MutRc<State<Done>>>::channel();
    cancel.unsubscribe();
    assert!(handle.now_or_never().is_some());
  }

  #[rxrust_macro::test]
  fn installation_deduplicates_waiters_and_wakes_outside_lock() {
    use std::{
      sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
      },
      task::Wake,
    };

    use crate::rc::MutArc;

    struct Waiter {
      state: MutArc<State<Done>>,
      wakes: AtomicUsize,
    }
    impl Wake for Waiter {
      fn wake(self: Arc<Self>) {
        assert!(self.state.try_rc_deref_mut().is_some());
        self.wakes.fetch_add(1, Ordering::SeqCst);
      }
    }
    let (install, [mut handle]) = SingleAssignment::<MutArc<_>>::channel();
    let waiter = Arc::new(Waiter { state: handle.0.clone(), wakes: AtomicUsize::new(0) });
    let waker = Waker::from(waiter.clone());
    let mut cx = Context::from_waker(&waker);
    assert!(Pin::new(&mut handle).poll(&mut cx).is_pending());
    assert!(Pin::new(&mut handle).poll(&mut cx).is_pending());
    install(Done);
    assert_eq!(waiter.wakes.load(Ordering::SeqCst), 1);
    assert!(Pin::new(&mut handle).poll(&mut cx).is_ready());
  }

  #[rxrust_macro::test]
  fn first_late_installation_is_cancelled_outside_lock() {
    let calls = Rc::new(Cell::new(0));
    let cancelled = calls.clone();
    let (install, [other, handle]) =
      SingleAssignment::<MutRc<State<crate::subscription::BoxedSubscription>>>::channel();
    handle.unsubscribe();
    install(crate::subscription::BoxedSubscription::new(ClosureSubscription(move || {
      assert!(other.is_closed());
      cancelled.set(cancelled.get() + 1);
    })));
    assert_eq!(calls.get(), 1);
  }

  #[rxrust_macro::test]
  fn repeated_cancellation_before_installation_is_idempotent() {
    let calls = Cell::new(0);
    let (install, [cancel, handle]) = SingleAssignment::<MutRc<_>>::channel();
    handle.unsubscribe();
    cancel.unsubscribe();
    install(ClosureSubscription(|| calls.set(calls.get() + 1)));
    assert_eq!(calls.get(), 1);
  }

  #[rxrust_macro::test]
  fn repeated_cancellation_after_installation_is_idempotent() {
    let calls = Cell::new(0);
    let (install, [first, second, handle]) = SingleAssignment::<MutRc<_>>::channel();
    install(ClosureSubscription(|| calls.set(calls.get() + 1)));
    first.unsubscribe();
    second.unsubscribe();
    assert!(handle.is_closed());
    assert_eq!(calls.get(), 1);
  }

  #[rxrust_macro::test]
  fn clone_shares_state_without_cloning_upstream() {
    struct Cloneable;
    impl Clone for Cloneable {
      fn clone(&self) -> Self { panic!("upstream value must not be cloned") }
    }
    impl Subscription for Cloneable {
      fn unsubscribe(self) {}
      fn is_closed(&self) -> bool { false }
    }
    let (install, [cancel, handle]) = SingleAssignment::<MutRc<_>>::channel();
    install(Cloneable);
    handle.clone().unsubscribe();
    assert!(handle.is_closed());
    assert!(cancel.is_closed());
  }

  #[rxrust_macro::test]
  fn cleanup_is_outside_lock_and_releases_captures() {
    let (install, [other, handle]) =
      SingleAssignment::<MutRc<State<crate::subscription::BoxedSubscription>>>::channel();
    let value = Rc::new(());
    let weak = Rc::downgrade(&value);
    use crate::subscription::IntoBoxedSubscription;
    install(
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
    let (install, [cancel, handle]) = SingleAssignment::<MutRc<_>>::channel();
    install(ClosureSubscription(|| calls.set(calls.get() + 1)));
    drop(cancel);
    drop(handle);
    assert_eq!(calls.get(), 0);
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
      let (install, [handle]) = SingleAssignment::<crate::rc::MutArc<_>>::channel();
      let barrier = Arc::new(Barrier::new(2));
      let ready = barrier.clone();
      let count = calls.clone();
      let thread = std::thread::spawn(move || {
        ready.wait();
        install(ClosureSubscription(move || {
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
