use smallvec::SmallVec;

use crate::rc::{MutArc, MutRc, RcDeref, RcDerefMut};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait Subscription {
  /// This allows deregistering an stream before it has finished receiving all
  /// events (i.e. before onCompleted is called).
  fn unsubscribe(self);

  /// detect if the subscription already be unsubscribed.
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

pub struct ZipSubscription<A, B> {
  a: A,
  b: B,
}

#[derive(Clone)]
pub struct MultiSubscription<'a>(
  MutRc<Option<SmallVec<[Option<BoxSubscription<'a>>; 1]>>>,
);
#[derive(Clone)]
pub struct MultiSubscriptionThreads(
  MutArc<Option<SmallVec<[Option<BoxSubscriptionThreads>; 1]>>>,
);

pub struct BoxSubscription<'a>(Box<dyn BoxSubscriptionInner + 'a>);
pub struct BoxSubscriptionThreads(Box<dyn BoxSubscriptionInner + Send>);

impl<A: Subscription, B: Subscription> ZipSubscription<A, B> {
  #[inline]
  pub fn new(a: A, b: B) -> Self {
    ZipSubscription { a, b }
  }
}

impl<U: Subscription, H: Subscription> Subscription for ZipSubscription<H, U> {
  fn unsubscribe(self) {
    self.a.unsubscribe();
    self.b.unsubscribe();
  }

  fn is_closed(&self) -> bool {
    self.b.is_closed()
  }
}

macro_rules! impl_multi_subscription {
  ($ty:ty, $box_ty: ty $(,$lf: lifetime)?) => {
    impl<$($lf)?> Subscription for $ty {
      fn unsubscribe(self) {
        let vec = self.0.rc_deref_mut().take();
        if let Some(vec) = vec {
          vec.into_iter().for_each(|u| {
            if let Some(unsub) = u {
              unsub.0.boxed_unsubscribe();
            }
          })
        }
      }

      fn is_closed(&self) -> bool {
        self.0.rc_deref().as_ref()
          .map_or(true, |m| {
            m.iter().all(|u| u.as_ref().map_or(true, |v| v.is_closed()))
          })
      }
    }

    impl<$($lf)?>  $ty {
      pub fn teardown_size(&self) -> usize {
        self.0.rc_deref_mut().as_mut().map_or(0, |vec| vec.len())
      }
      pub fn append(&mut self, v: $box_ty) {
        if let Some(vec) = self.0.rc_deref_mut().as_mut() {
          vec.push(Some(v));
        }
      }
      pub fn retain(&mut self) {
        if let Some(vec) = self.0.rc_deref_mut().as_mut() {
          vec.retain(|v| v.is_some());
        }
      }
    }
  };
}

impl Subscription for () {
  #[inline]
  fn unsubscribe(self) {}

  #[inline]
  fn is_closed(&self) -> bool {
    true
  }
}

impl_multi_subscription!(MultiSubscription<'a>, BoxSubscription<'a>, 'a);
impl_multi_subscription!(MultiSubscriptionThreads, BoxSubscriptionThreads);

impl<'a> Default for MultiSubscription<'a> {
  fn default() -> Self {
    Self(MutRc::own(Some(<_>::default())))
  }
}

impl Default for MultiSubscriptionThreads {
  fn default() -> Self {
    Self(MutArc::own(Some(<_>::default())))
  }
}

impl<T: Subscription> SubscriptionGuard<T> {
  /// Wraps an existing subscription with a guard to enable RAII behavior for
  /// it.
  pub fn new(subscription: T) -> SubscriptionGuard<T> {
    SubscriptionGuard(Some(subscription))
  }
}

impl<T: Subscription> Drop for SubscriptionGuard<T> {
  fn drop(&mut self) {
    if let Some(u) = self.0.take() {
      u.unsubscribe()
    }
  }
}

impl<T, S> Subscription for T
where
  T: RcDerefMut<Target = Option<S>> + RcDeref<Target = Option<S>>,
  S: Subscription,
{
  #[inline]
  fn unsubscribe(self) {
    if let Some(u) = self.rc_deref_mut().take() {
      u.unsubscribe()
    }
  }

  fn is_closed(&self) -> bool {
    self.rc_deref().is_none()
  }
}
trait BoxSubscriptionInner {
  fn boxed_unsubscribe(self: Box<Self>);

  fn boxed_is_closed(&self) -> bool;
}

impl<T: Subscription> BoxSubscriptionInner for T {
  #[inline]
  fn boxed_unsubscribe(self: Box<Self>) {
    self.unsubscribe()
  }

  #[inline]
  fn boxed_is_closed(&self) -> bool {
    (*self).is_closed()
  }
}

impl<'a> BoxSubscription<'a> {
  #[inline]
  pub fn new(subscription: impl Subscription + 'a) -> Self {
    Self(Box::new(subscription))
  }
}

impl BoxSubscriptionThreads {
  #[inline]
  pub fn new(subscription: impl Subscription + Send + 'static) -> Self {
    Self(Box::new(subscription))
  }
}

impl<'a> Subscription for BoxSubscription<'a> {
  #[inline]
  fn unsubscribe(self) {
    self.0.boxed_unsubscribe()
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.0.boxed_is_closed()
  }
}

impl Subscription for BoxSubscriptionThreads {
  #[inline]
  fn unsubscribe(self) {
    self.0.boxed_unsubscribe()
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.0.boxed_is_closed()
  }
}

#[cfg(test)]
mod test {

  use super::*;
  #[test]
  fn add_remove_for_local() {
    let mut local = MultiSubscription::default();
    let l1 = MultiSubscription::default();
    let l2 = MultiSubscription::default();
    let l3 = MultiSubscription::default();
    local.append(BoxSubscription::new(l1));
    assert_eq!(local.teardown_size(), 1);
    local.append(BoxSubscription::new(l2));
    assert_eq!(local.teardown_size(), 2);
    local.append(BoxSubscription::new(l3));
    assert_eq!(local.teardown_size(), 3);
  }

  #[test]
  fn add_remove_for_shared() {
    let mut shared = MultiSubscriptionThreads::default();
    let l1 = MultiSubscriptionThreads::default();
    let l2 = MultiSubscriptionThreads::default();
    let l3 = MultiSubscriptionThreads::default();
    shared.append(BoxSubscriptionThreads::new(l1));
    assert_eq!(shared.teardown_size(), 1);
    shared.append(BoxSubscriptionThreads::new(l2));
    assert_eq!(shared.teardown_size(), 2);
    shared.append(BoxSubscriptionThreads::new(l3));
    assert_eq!(shared.teardown_size(), 3);
  }

  #[test]
  fn fix_box_subscription_no_proxy() {
    let a = BoxSubscription::new(());
    assert!(a.is_closed());
  }
}
