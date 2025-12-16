use super::Subscription;

/// Helper trait for calling unsubscribe on boxed trait objects
///
/// Since `Subscription::unsubscribe(self)` requires `Sized`, we need this
/// workaround trait to enable `Box<dyn Subscription>` to call unsubscribe.
pub trait BoxedSubscriptionInner {
  fn boxed_unsubscribe(self: Box<Self>);
  fn boxed_is_closed(&self) -> bool;
}

impl<T: Subscription> BoxedSubscriptionInner for T {
  #[inline]
  fn boxed_unsubscribe(self: Box<Self>) { (*self).unsubscribe() }

  #[inline]
  fn boxed_is_closed(&self) -> bool { self.is_closed() }
}

/// A boxed subscription for local (single-threaded) contexts.
///
/// This allows storing heterogeneous subscriptions in collections
/// and enables type erasure for subscription management.
///
/// # Why `'static`?
///
/// Subscriptions are **control handles**, not data views. They are designed to
/// be stored, moved, and called at an arbitrary later time (e.g., in a
/// `MergeAllState` or a `Subject`).
///
/// If a Subscription carried a lifetime `'a`, it would mean it holds a borrow
/// of some data. This would severely restrict where it can be stored and when
/// it can be dropped, effectively coupling the subscription's lifecycle to
/// the borrowed data's scope. By enforcing `'static` (via `Box<dyn ... +
/// 'static>`), we ensure subscriptions are self-contained and "owned", allowing
/// them to be managed freely.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// // Create subscriptions of different types
/// let sub1 = BoxedSubscription::new(());
/// let sub2 = BoxedSubscription::new(());
///
/// // Store them in a collection
/// let subs: Vec<BoxedSubscription> = vec![sub1, sub2];
///
/// // Unsubscribe all
/// for sub in subs {
///   sub.unsubscribe();
/// }
/// ```
pub struct BoxedSubscription(Box<dyn BoxedSubscriptionInner>);

/// A boxed subscription for shared (multi-threaded) contexts.
///
/// This allows storing heterogeneous subscriptions in collections
/// across thread boundaries.
///
/// # Why `'static`?
///
/// Similar to `BoxedSubscription`, shared subscriptions must be `'static`
/// to be truly independent control handles. In multi-threaded contexts, this is
/// even more critical as valid lifetimes across threads are typically `'static`
/// (required for `Send` in many cases, though `scoped` threads exist, `rxRust`
/// targets general async/sync usage).
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// // Create a thread-safe boxed subscription
/// let sub = BoxedSubscriptionSend::new(());
/// ```
pub struct BoxedSubscriptionSend(Box<dyn BoxedSubscriptionInner + Send>);

impl BoxedSubscription {
  /// Create a new boxed subscription from any subscription type.
  #[inline]
  pub fn new(subscription: impl Subscription + 'static) -> Self { Self(Box::new(subscription)) }
}

impl BoxedSubscriptionSend {
  /// Create a new thread-safe boxed subscription from any Send subscription
  /// type.
  #[inline]
  pub fn new(subscription: impl Subscription + Send + 'static) -> Self {
    Self(Box::new(subscription))
  }
}

// ==================== IntoBoxedSubscription Trait ====================

/// Trait for converting a subscription into a boxed subscription.
///
/// This trait provides a convenient way to convert any `Subscription` into
/// a boxed subscription without trait implementation conflicts.
///
/// # Example
///
/// ```rust
/// use rxrust::prelude::*;
///
/// fn store_subscription<T: IntoBoxedSubscription<BoxedSubscription>>(sub: T) -> BoxedSubscription {
///   sub.into_boxed()
/// }
/// ```
pub trait IntoBoxedSubscription<Target> {
  /// Convert this subscription into a boxed subscription.
  fn into_boxed(self) -> Target;
}

/// Blanket implementation for BoxedSubscription.
impl<T: Subscription + 'static> IntoBoxedSubscription<BoxedSubscription> for T {
  #[inline]
  fn into_boxed(self) -> BoxedSubscription { BoxedSubscription::new(self) }
}

/// Blanket implementation for BoxedSubscriptionSend.
impl<T: Subscription + Send + 'static> IntoBoxedSubscription<BoxedSubscriptionSend> for T {
  #[inline]
  fn into_boxed(self) -> BoxedSubscriptionSend { BoxedSubscriptionSend::new(self) }
}

impl Subscription for BoxedSubscription {
  #[inline]
  fn unsubscribe(self) { self.0.boxed_unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.0.boxed_is_closed() }
}

impl Subscription for BoxedSubscriptionSend {
  #[inline]
  fn unsubscribe(self) { self.0.boxed_unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.0.boxed_is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;

  /// A mock subscription for testing
  struct MockSubscription {
    closed: Rc<RefCell<bool>>,
  }

  impl MockSubscription {
    fn new() -> (Self, Rc<RefCell<bool>>) {
      let closed = Rc::new(RefCell::new(false));
      (Self { closed: closed.clone() }, closed)
    }
  }

  impl Subscription for MockSubscription {
    fn unsubscribe(self) { *self.closed.borrow_mut() = true; }

    fn is_closed(&self) -> bool { *self.closed.borrow() }
  }

  #[rxrust_macro::test]
  fn test_local_boxed_subscription() {
    let (mock, closed) = MockSubscription::new();
    let boxed = BoxedSubscription::new(mock);

    assert!(!boxed.is_closed());
    boxed.unsubscribe();
    assert!(*closed.borrow());
  }

  #[rxrust_macro::test]
  fn test_local_boxed_subscription_with_unit() {
    let boxed = BoxedSubscription::new(());
    assert!(boxed.is_closed()); // Unit subscription is always closed
    boxed.unsubscribe(); // Should not panic
  }

  #[rxrust_macro::test]
  fn test_boxed_subscription_in_collection() {
    let (mock1, closed1) = MockSubscription::new();
    let (mock2, closed2) = MockSubscription::new();

    let subs: Vec<BoxedSubscription> =
      vec![BoxedSubscription::new(mock1), BoxedSubscription::new(mock2)];

    assert!(!*closed1.borrow());
    assert!(!*closed2.borrow());

    for sub in subs {
      sub.unsubscribe();
    }

    assert!(*closed1.borrow());
    assert!(*closed2.borrow());
  }
}
