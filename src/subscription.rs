use smallvec::SmallVec;
use std::{
  any::Any,
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait SubscriptionLike {
  /// This allows deregistering an stream before it has finished receiving all
  /// events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);

  fn is_closed(&self) -> bool;
}

// todo provide LocalRef/SharedRef generic type for all subscription type.

#[derive(Clone, Default)]
pub struct LocalSubscription(
  Rc<RefCell<MultiSubscription<Box<dyn SubscriptionLike>>>>,
);

impl LocalSubscription {
  pub fn add<S: SubscriptionLike + 'static>(&self, subscription: S) {
    if !self.is_same(&subscription) {
      self.0.borrow_mut().add(Box::new(subscription))
    }
  }

  fn is_same(&self, other: &dyn Any) -> bool {
    if let Some(other) = other.downcast_ref::<Self>() {
      Rc::ptr_eq(&self.0, &other.0)
    } else {
      false
    }
  }
}

impl TearDownSize for LocalSubscription {
  fn teardown_size(&self) -> usize { self.0.borrow().teardown.len() }
}

pub trait TearDownSize: SubscriptionLike {
  fn teardown_size(&self) -> usize;
}

impl SubscriptionLike for LocalSubscription {
  #[inline]
  fn unsubscribe(&mut self) { self.0.unsubscribe() }
  #[inline]
  fn is_closed(&self) -> bool { self.0.is_closed() }
}

impl<S: SubscriptionLike + ?Sized> SubscriptionLike for Box<S> {
  #[inline]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe() }
  #[inline]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
}
#[derive(Clone, Default)]
pub struct SharedSubscription(
  Arc<Mutex<MultiSubscription<Box<dyn SubscriptionLike + Send + Sync>>>>,
);

impl SharedSubscription {
  pub fn add<S: SubscriptionLike + Send + Sync + 'static>(
    &self,
    subscription: S,
  ) {
    if !self.is_same(&subscription) {
      self.0.lock().unwrap().add(Box::new(subscription));
    }
  }

  fn is_same(&self, other: &dyn Any) -> bool {
    if let Some(other) = other.downcast_ref::<Self>() {
      Arc::ptr_eq(&self.0, &other.0)
    } else {
      false
    }
  }
}

impl TearDownSize for SharedSubscription {
  fn teardown_size(&self) -> usize { self.0.lock().unwrap().teardown.len() }
}

impl SubscriptionLike for SharedSubscription {
  #[inline]
  fn unsubscribe(&mut self) { self.0.unsubscribe(); }
  #[inline]
  fn is_closed(&self) -> bool { self.0.is_closed() }
}

pub struct MultiSubscription<T> {
  closed: bool,
  teardown: SmallVec<[T; 1]>,
}

impl<T: SubscriptionLike> SubscriptionLike for MultiSubscription<T> {
  #[inline(always)]
  fn is_closed(&self) -> bool { self.closed }

  fn unsubscribe(&mut self) {
    if !self.closed {
      self.closed = true;
      for v in &mut self.teardown {
        v.unsubscribe();
      }
    }
  }
}

impl<T: SubscriptionLike> MultiSubscription<T> {
  fn add(&mut self, mut v: T) {
    if self.closed {
      v.unsubscribe();
    } else {
      self.teardown.retain(|v| !v.is_closed());
      self.teardown.push(v);
    }
  }
}

impl<T> Default for MultiSubscription<T> {
  fn default() -> Self {
    MultiSubscription {
      closed: false,
      teardown: SmallVec::new(),
    }
  }
}

/// Wrapper around a subscription which provides the
/// `unsubscribe_when_dropped()` method.
pub struct SubscriptionWrapper<T: SubscriptionLike>(pub(crate) T);

impl<T: SubscriptionLike> SubscriptionWrapper<T> {
  /// Activates "RAII" behavior for this subscription. That means
  /// `unsubscribe()` will be called automatically as soon as the returned
  /// value goes out of scope.
  ///
  /// **Attention:** If you don't assign the return value to a variable,
  /// `unsubscribe()` is called immediately, which is probably not what you
  /// want!
  pub fn unsubscribe_when_dropped(self) -> SubscriptionGuard<T> {
    SubscriptionGuard(self.0)
  }

  /// Consumes this wrapper and returns the underlying subscription.
  pub fn into_inner(self) -> T { self.0 }
}

impl<T: SubscriptionLike> SubscriptionLike for SubscriptionWrapper<T> {
  #[inline]
  fn is_closed(&self) -> bool { self.0.is_closed() }
  #[inline]
  fn unsubscribe(&mut self) { self.0.unsubscribe() }
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
#[derive(Debug)]
#[must_use]
pub struct SubscriptionGuard<T: SubscriptionLike>(pub(crate) T);

impl<T: SubscriptionLike> SubscriptionGuard<T> {
  /// Wraps an existing subscription with a guard to enable RAII behavior for
  /// it.
  pub fn new(subscription: T) -> SubscriptionGuard<T> {
    SubscriptionGuard(subscription)
  }
}

impl<T: SubscriptionLike> Drop for SubscriptionGuard<T> {
  #[inline]
  fn drop(&mut self) { self.0.unsubscribe() }
}

#[derive(Default, Clone)]
pub struct SingleSubscription(bool);

impl SubscriptionLike for SingleSubscription {
  #[inline]
  fn unsubscribe(&mut self) { self.0 = true; }

  #[inline]
  fn is_closed(&self) -> bool { self.0 }
}

pub struct ProxySubscription<T: SubscriptionLike>(Option<T>);

impl<T: SubscriptionLike> ProxySubscription<T> {
  pub fn proxy(&mut self, proxy: T) -> Option<T> { self.0.replace(proxy) }
}

impl<T: SubscriptionLike> SubscriptionLike for ProxySubscription<T> {
  fn unsubscribe(&mut self) {
    if let Some(s) = &mut self.0 {
      s.unsubscribe()
    }
  }

  fn is_closed(&self) -> bool {
    self.0.as_ref().map_or(false, |s| s.is_closed())
  }
}

impl<T: SubscriptionLike> SubscriptionLike for Arc<Mutex<T>> {
  #[inline]
  fn unsubscribe(&mut self) { self.lock().unwrap().unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.lock().unwrap().is_closed() }
}

impl<T: SubscriptionLike> SubscriptionLike for Rc<RefCell<T>> {
  #[inline]
  fn unsubscribe(&mut self) { self.borrow_mut().unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.borrow().is_closed() }
}

impl<T: SubscriptionLike> Default for ProxySubscription<T> {
  fn default() -> Self { Self(Default::default()) }
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn add_remove_for_local() {
    let local = LocalSubscription::default();
    let l1 = LocalSubscription::default();
    let l2 = LocalSubscription::default();
    let l3 = LocalSubscription::default();
    local.add(l1);
    assert_eq!(local.0.borrow().teardown.len(), 1);
    local.add(l2);
    assert_eq!(local.0.borrow().teardown.len(), 2);
    local.add(l3);
    assert_eq!(local.0.borrow().teardown.len(), 3);
  }

  #[test]
  fn add_remove_for_shared() {
    let local = SharedSubscription::default();
    let l1 = SharedSubscription::default();
    let l2 = SharedSubscription::default();
    let l3 = SharedSubscription::default();
    local.add(l1);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 1);
    local.add(l2);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 2);
    local.add(l3);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 3);
  }
}
