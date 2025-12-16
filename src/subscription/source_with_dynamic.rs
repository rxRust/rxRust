//! SourceWithDynamicSubs subscription combinator
//!
//! A generic subscription that combines a source subscription with
//! a DynamicSubscriptions container. Used by operators like ObserveOn,
//! MergeAll, and Delay that manage multiple dynamic subscriptions.

use super::{DynamicSubscriptions, Subscription};
use crate::context::RcDerefMut;

/// A subscription combining a source subscription with dynamic subscriptions.
///
/// When unsubscribed, all dynamic subscriptions are drained and unsubscribed
/// first, then the source subscription is unsubscribed.
///
/// # Type Parameters
///
/// - `U`: The source subscription type
/// - `S`: The RcDerefMut container for DynamicSubscriptions
///
/// # Examples
///
/// ```ignore
/// type ObserveOnSubscription<U, S> = SourceWithDynamicSubs<U, S>;
/// type MergeAllSubscription<U, S> = SourceWithDynamicSubs<U, S>;
/// type DelaySubscription<U, S> = SourceWithDynamicSubs<U, S>;
/// ```
pub struct SourceWithDynamicSubs<U, S> {
  pub source: U,
  pub subs: S,
}

impl<U, S> SourceWithDynamicSubs<U, S> {
  /// Create a new SourceWithDynamicSubs subscription
  #[inline]
  pub fn new(source: U, subs: S) -> Self { Self { source, subs } }
}

impl<U, S, H> Subscription for SourceWithDynamicSubs<U, S>
where
  U: Subscription,
  S: RcDerefMut<Target = DynamicSubscriptions<H>>,
  H: Subscription,
{
  fn unsubscribe(self) {
    // Unsubscribe source first
    self.source.unsubscribe();
    // Then drain and unsubscribe all dynamic subs
    self.subs.rc_deref_mut().unsubscribe_all();
  }

  fn is_closed(&self) -> bool { self.source.is_closed() }
}
