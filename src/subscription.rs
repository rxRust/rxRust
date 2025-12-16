//! Subscription trait and implementations
//!
//! This module contains the Subscription trait for managing the lifecycle
//! of subscriptions.

pub mod boxed;
pub mod dynamic;
pub mod either;
pub mod source_with_dynamic;
pub mod source_with_handle;
pub mod tuple;

pub use boxed::{BoxedSubscription, BoxedSubscriptionSend, IntoBoxedSubscription};
pub use dynamic::DynamicSubscriptions;
pub use either::EitherSubscription;
pub use source_with_dynamic::SourceWithDynamicSubs;
pub use source_with_handle::SourceWithHandle;
pub use tuple::TupleSubscription;

/// A functional adapter that turns a closure into a `Subscription`.
///
/// This struct allows users to define teardown/unsubscription logic using a
/// simple closure, without needing to define a custom struct that implements
/// the `Subscription` trait. It is particularly useful when working with the
/// [`create`](crate::observable::ObservableFactory::create) operator, where you
/// often need to return a cleanup action.
///
/// # Why is this needed?
///
/// Rust's orphan rules and type inference limitations prevent us from directly
/// implementing `Subscription` for all `FnOnce()` closures globally.
/// `ClosureSubscription` serves as an explicit wrapper to tell the compiler:
/// "Treat this closure as a Subscription".
///
/// # Zero-Cost Abstraction
///
/// This is a newtype wrapper that compiles down to a direct function call.
/// There is no runtime overhead compared to calling the closure directly.
///
/// # Example
///
/// ```rust
/// use std::convert::Infallible;
///
/// use rxrust::{prelude::*, subscription::ClosureSubscription};
///
/// Local::create::<(), Infallible, _, _>(|_emitter| {
///   println!("Subscribed");
///
///   // Return a closure wrapped in ClosureSubscription as the teardown logic
///   ClosureSubscription(move || {
///     println!("Unsubscribed - cleaning up resources");
///   })
/// });
/// ```
pub struct ClosureSubscription<F>(pub F);

impl<F> Subscription for ClosureSubscription<F>
where
  F: FnOnce(),
{
  fn unsubscribe(self) { (self.0)() }

  fn is_closed(&self) -> bool { false }
}

/// An RAII implementation of a "scoped subscribed" of a subscription.
/// When this structure is dropped (falls out of scope), the subscription will
/// be unsubscribed.
///
/// Implements the [must_use](
/// https://doc.rust-lang.org/reference/attributes/diagnostics.html
/// #the-must_use-attribute)
/// attribute
///
/// If you want to drop it immediately, wrap it in its own scope
#[must_use]
pub struct SubscriptionGuard<T: Subscription>(pub(crate) Option<T>);

impl<T: Subscription> SubscriptionGuard<T> {
  /// Wraps an existing subscription with a guard to enable RAII behavior for
  /// it.
  pub fn new(subscription: T) -> SubscriptionGuard<T> { SubscriptionGuard(Some(subscription)) }
}

impl<T: Subscription> Drop for SubscriptionGuard<T> {
  fn drop(&mut self) {
    if let Some(u) = self.0.take() {
      u.unsubscribe()
    }
  }
}

/// Subscription trait for managing observable subscriptions
///
/// Provides methods to cancel a subscription and check its status.
/// Uses move semantics for `unsubscribe` to match the terminal nature of the
/// operation, consistent with Observer's `error(self)` and `complete(self)`
/// methods.
pub trait Subscription {
  /// Cancel the subscription (terminal operation, consumes self)
  fn unsubscribe(self);

  /// Check if the subscription is closed (completed or unsubscribed)
  fn is_closed(&self) -> bool;

  /// Activates "RAII" behavior for this subscription. That means
  /// `unsubscribe()` will be called automatically as soon as the returned
  /// value goes out of scope.
  ///
  /// **Attention:** If you don't assign the return value to a variable,
  /// `unsubscribe()` is called immediately, which is probably not what you
  /// want!
  fn unsubscribe_when_dropped(self) -> SubscriptionGuard<Self>
  where
    Self: Sized,
  {
    SubscriptionGuard::new(self)
  }
}

/// Unit subscription - always closed, does nothing
impl Subscription for () {
  fn unsubscribe(self) {}

  fn is_closed(&self) -> bool { true }
}

/// Option subscription - None is closed, Some delegates to inner
impl<P: Subscription> Subscription for Option<P> {
  fn unsubscribe(self) {
    if let Some(inner) = self {
      inner.unsubscribe()
    }
  }

  fn is_closed(&self) -> bool {
    match self {
      Some(inner) => inner.is_closed(),
      None => true,
    }
  }
}

use crate::context::{MutArc, MutRc, RcDeref, RcDerefMut};

impl<P: Subscription> Subscription for MutArc<Option<P>> {
  fn unsubscribe(self) {
    let Some(inner) = self.rc_deref_mut().take() else {
      return;
    };
    inner.unsubscribe()
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_none() }
}

impl<P: Subscription> Subscription for MutRc<Option<P>> {
  fn unsubscribe(self) {
    let Some(inner) = self.rc_deref_mut().take() else {
      return;
    };
    inner.unsubscribe()
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_none() }
}

#[cfg(test)]
mod test {
  use std::{cell::RefCell, rc::Rc};

  use super::*;

  #[rxrust_macro::test]
  fn test_subscription_guard_drop() {
    let unsubscribed = Rc::new(RefCell::new(false));
    let unsubscribed_clone = unsubscribed.clone();

    struct TestSubscription {
      is_closed: bool,
      unsubscribed_flag: Rc<RefCell<bool>>,
    }

    impl Subscription for TestSubscription {
      fn unsubscribe(mut self) {
        *self.unsubscribed_flag.borrow_mut() = true;
        self.is_closed = true;
      }

      fn is_closed(&self) -> bool { self.is_closed }
    }

    let sub = TestSubscription { is_closed: false, unsubscribed_flag: unsubscribed_clone };

    assert!(!*unsubscribed.borrow());
    {
      let _guard = sub.unsubscribe_when_dropped();
      assert!(!*unsubscribed.borrow());
    }
    // When _guard goes out of scope, unsubscribe should be called
    assert!(*unsubscribed.borrow());
  }
}
