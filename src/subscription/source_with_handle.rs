//! SourceWithHandle subscription combinator
//!
//! A generic subscription that combines a source subscription with an
//! additional cancelable handle. Used by operators like Debounce, Throttle,
//! and Delay that need to cancel scheduled tasks on unsubscribe.

use super::Subscription;

/// A subscription combining a source subscription with a cancelable handle.
///
/// When unsubscribed, both the handle and the source are unsubscribed.
/// The `is_closed` check delegates to the source subscription.
///
/// # Type Parameters
///
/// - `U`: The source subscription type
/// - `H`: The handle subscription type (e.g., wrapped TaskHandle)
///
/// # Examples
///
/// ```ignore
/// type DebounceSubscription<U, H> = SourceWithHandle<U, H>;
/// type ThrottleSubscription<U, H> = SourceWithHandle<U, H>;
/// type DelaySubscription<U, H> = SourceWithHandle<U, H>;
/// ```
pub struct SourceWithHandle<U, H> {
  pub source: U,
  pub handle: H,
}

impl<U, H> SourceWithHandle<U, H> {
  /// Create a new SourceWithHandle subscription
  #[inline]
  pub fn new(source: U, handle: H) -> Self { Self { source, handle } }
}

impl<U, H> Subscription for SourceWithHandle<U, H>
where
  U: Subscription,
  H: Subscription,
{
  fn unsubscribe(self) {
    self.handle.unsubscribe();
    self.source.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.source.is_closed() }
}
