use std::sync::{
  Arc, Mutex,
  atomic::{AtomicBool, Ordering},
};

use crate::{observer::Observer, subscription::DynamicSubscriptions};

/// Subscribers container using DynamicSubscriptions for ID-based management.
///
/// This struct holds the list of observers subscribed to the Subject.
/// It wraps `DynamicSubscriptions` to provide ID-based add/remove and
/// adds Subject-specific broadcast functionality.
///
/// # Design Rationale
///
/// - **DynamicSubscriptions**: Delegates ID-based storage and removal to the
///   common `DynamicSubscriptions` abstraction.
/// - **Clone Optimization**: When broadcasting, the last observer receives the
///   moved value instead of a clone, reducing unnecessary allocations.
///
/// # Type Parameters
///
/// - `Ob`: The observer type stored in this container.
pub struct Subscribers<Ob> {
  inner: DynamicSubscriptions<Subscriber<Ob>>,
  pub(crate) count: Arc<Mutex<usize>>,
}

impl<Ob> Default for Subscribers<Ob> {
  fn default() -> Self { Self { inner: DynamicSubscriptions::default(), count: Arc::default() } }
}

impl<Ob> Subscribers<Ob> {
  /// Add an observer and return its unique ID.
  #[inline]
  pub fn add(&mut self, observer: Ob) -> usize {
    let membership = Membership::new(self.count.clone());
    self.add_registered(observer, membership)
  }

  pub(crate) fn add_registered(&mut self, observer: Ob, membership: Arc<Membership>) -> usize {
    self
      .inner
      .add(Subscriber { observer, membership })
  }

  /// Insert an observer with a pre-allocated ID.
  #[inline]
  pub fn insert(&mut self, id: usize, observer: Ob) {
    let membership = Membership::new(self.count.clone());
    self
      .inner
      .insert(id, Subscriber { observer, membership });
  }

  /// Remove an observer by ID.
  #[inline]
  pub fn remove(&mut self, id: usize) -> Option<Ob> {
    self.inner.remove(id).map(|entry| {
      entry.membership.close();
      entry.observer
    })
  }

  /// Check if an ID exists.
  #[inline]
  pub fn contains(&self, id: usize) -> bool { self.inner.contains(id) }
}

impl<Ob> Subscribers<Ob> {
  /// Broadcast value to all observers with optimal cloning.
  ///
  /// This method implements a performance optimization where the value is
  /// cloned for all observers except the last one, which receives the moved
  /// value. This eliminates unnecessary cloning while maintaining correct
  /// semantics.
  ///
  /// # Performance Characteristics
  ///
  /// - **n subscribers**: n-1 clones + 1 move (optimal for multicasting)
  /// - **1 subscriber**: 0 clones + 1 move (no overhead)
  /// - **0 subscribers**: no-op (early return optimization)
  ///
  /// # Type Constraints
  ///
  /// - `Ob`: Must implement `Observer<Item, Err>`
  /// - `Item`: Must implement `Clone` for multicasting
  pub(crate) fn broadcast_value<Item, Err>(&mut self, value: Item)
  where
    Ob: Observer<Item, Err>,
    Item: Clone,
  {
    let mut iter = self.inner.iter_mut().peekable();
    while let Some(observer) = iter.next() {
      if iter.peek().is_some() {
        observer.next(value.clone());
      } else {
        observer.next(value);
        break;
      }
    }
  }

  /// Broadcast mutable reference to all observers sequentially.
  ///
  /// This method leverages Rust's re-borrowing mechanism to safely pass
  /// the same `&mut T` to multiple observers in sequence. Each observer
  /// can modify the value, and the changes are visible to subsequent observers.
  ///
  /// # Type Constraints
  ///
  /// - `Ob`: Must implement `Observer<&'a mut T, Err>` for all lifetimes `'a`
  ///   (HRTB)
  ///
  /// # Safety
  ///
  /// This is completely safe because:
  /// 1. Re-borrowing creates a new mutable borrow with a shorter lifetime
  /// 2. Each observer call completes before the next one begins
  /// 3. There's no concurrent access to the mutable reference
  pub(crate) fn broadcast_mut_ref<T, Err>(&mut self, value: &mut T)
  where
    Ob: for<'a> Observer<&'a mut T, Err>,
  {
    for observer in self.inner.iter_mut() {
      observer.next(value);
    }
  }

  /// Broadcast error to all observers and clear the subscriber list.
  ///
  /// This method drains all observers and sends the error to each.
  /// The error is cloned for all observers except the last one.
  ///
  /// # Post-condition
  ///
  /// After calling this method, `self.inner` is empty.
  ///
  /// # Type Constraints
  ///
  /// - `Ob`: Must implement `Observer<Item, Err>`
  /// - `Err`: Must implement `Clone` for multicasting
  pub(crate) fn broadcast_error<Item, Err>(&mut self, err: Err)
  where
    Ob: Observer<Item, Err>,
    Err: Clone,
  {
    let mut iter = self.inner.drain().peekable();
    while let Some(observer) = iter.next() {
      if iter.peek().is_some() {
        observer.error(err.clone());
      } else {
        observer.error(err);
        break;
      }
    }
  }

  /// Broadcast completion to all observers and clear the subscriber list.
  ///
  /// This method drains all observers and completes each one.
  ///
  /// # Post-condition
  ///
  /// After calling this method, `self.inner` is empty.
  ///
  /// # Type Constraints
  ///
  /// - `Ob`: Must implement `Observer<Item, Err>`
  pub(crate) fn broadcast_complete<Item, Err>(&mut self)
  where
    Ob: Observer<Item, Err>,
  {
    for observer in self.inner.drain() {
      observer.complete();
    }
  }
}

/// Logical membership is independent of deferred observer storage changes.
/// Neither dropping this token nor a subscription handle cancels membership.
pub(crate) struct Membership {
  closed: AtomicBool,
  count: Arc<Mutex<usize>>,
}

impl Membership {
  pub(crate) fn new(count: Arc<Mutex<usize>>) -> Arc<Self> {
    *count.lock().unwrap() += 1;
    Arc::new(Self { closed: AtomicBool::new(false), count })
  }

  pub(crate) fn is_closed(&self) -> bool { self.closed.load(Ordering::SeqCst) }

  pub(crate) fn close(&self) -> bool {
    // Concurrent terminal delivery and cancellation must both return with the
    // count updated. No observer code runs under this short-lived lock.
    let mut count = self.count.lock().unwrap();
    if self.closed.swap(true, Ordering::SeqCst) {
      return false;
    }
    *count -= 1;
    true
  }
}

struct Subscriber<O> {
  observer: O,
  membership: Arc<Membership>,
}

impl<I, E, O: Observer<I, E>> Observer<I, E> for Subscriber<O> {
  fn next(&mut self, value: I) {
    if !self.membership.is_closed() {
      self.observer.next(value);
    }
  }
  fn error(self, err: E) {
    if self.membership.close() {
      self.observer.error(err);
    }
  }
  fn complete(self) {
    if self.membership.close() {
      self.observer.complete();
    }
  }
  fn is_closed(&self) -> bool { self.membership.is_closed() || self.observer.is_closed() }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use super::*;

  #[test]
  fn cancellation_and_termination_release_membership_once() {
    use std::sync::Barrier;
    for _ in 0..64 {
      let count = Arc::new(Mutex::new(0));
      let membership = Membership::new(count.clone());
      let other = membership.clone();
      let barrier = Arc::new(Barrier::new(2));
      let ready = barrier.clone();
      let thread = std::thread::spawn(move || {
        ready.wait();
        other.close()
      });
      barrier.wait();
      let closed = membership.close();
      assert_ne!(closed, thread.join().unwrap());
      assert_eq!(*count.lock().unwrap(), 0);
      assert!(!membership.close());
      assert_eq!(*count.lock().unwrap(), 0);
    }
  }
}
