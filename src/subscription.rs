use crate::prelude::*;
use crate::util;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait SubscriptionLike {
  /// This allows deregistering an stream before it has finished receiving all
  /// events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);

  fn is_closed(&self) -> bool;

  fn inner_addr(&self) -> *const ();
}

#[derive(Clone, Default)]
pub struct LocalSubscription(Rc<RefCell<Inner<Box<dyn SubscriptionLike>>>>);

pub(crate) macro subscription_proxy_impl(
  $t: ty,  $host_ty: ident, $name:tt,
  <$($generics: ident  $(: $bound: tt)?),*>) {
  impl<$($generics $(:$bound)?),*> SubscriptionLike for $t
  where
    $host_ty: SubscriptionLike,
  {
    #[inline(always)]
    fn unsubscribe(&mut self) { self.$name.unsubscribe(); }
    #[inline(always)]
    fn is_closed(&self) -> bool { self.$name.is_closed() }
    #[inline(always)]
    fn inner_addr(&self) -> *const () { self.$name.inner_addr() }
  }
}

impl LocalSubscription {
  pub fn add<S: SubscriptionLike + 'static>(&mut self, subscription: S) {
    if self.inner_addr() != subscription.inner_addr() {
      self.0.borrow_mut().add(Box::new(subscription))
    }
  }

  pub fn remove(&mut self, subscription: &dyn SubscriptionLike) {
    self.0.borrow_mut().remove(subscription);
  }
}

impl TearDownSize for LocalSubscription {
  fn teardown_size(&self) -> usize { self.0.borrow().teardown.len() }
}

pub trait TearDownSize: SubscriptionLike {
  fn teardown_size(&self) -> usize;
}

impl SubscriptionLike for LocalSubscription {
  fn unsubscribe(&mut self) {
    let mut inner = self.0.borrow_mut();
    inner.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.0.borrow_mut().is_closed() }

  fn inner_addr(&self) -> *const () { self.0.as_ptr() as *const () }
}

#[derive(Clone, Default)]
pub struct SharedSubscription(
  Arc<Mutex<Inner<Box<dyn SubscriptionLike + Send + Sync>>>>,
);

impl SharedSubscription {
  pub fn add<S: SubscriptionLike + Send + Sync + 'static>(
    &mut self,
    subscription: S,
  ) {
    if self.inner_addr() != subscription.inner_addr() {
      self.0.lock().unwrap().add(Box::new(subscription));
    }
  }

  pub fn remove(&mut self, subscription: &dyn SubscriptionLike) {
    self.0.lock().unwrap().remove(subscription);
  }
}

impl TearDownSize for SharedSubscription {
  fn teardown_size(&self) -> usize { self.0.lock().unwrap().teardown.len() }
}

impl SubscriptionLike for SharedSubscription {
  fn unsubscribe(&mut self) {
    let mut inner = self.0.lock().unwrap();
    inner.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.0.lock().unwrap().is_closed() }

  fn inner_addr(&self) -> *const () {
    let inner = self.0.lock().unwrap();
    let pointer = &*inner as *const _;
    pointer as *const ()
  }
}

pub trait Publisher<Item, Err>: Observer<Item, Err> + SubscriptionLike {}

impl<Item, Err, T> Publisher<Item, Err> for T where
  T: Observer<Item, Err> + SubscriptionLike
{
}

struct Inner<T> {
  closed: bool,
  teardown: SmallVec<[T; 1]>,
}

impl<T: SubscriptionLike> Inner<T> {
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

  fn add(&mut self, mut v: T) {
    if self.closed {
      v.unsubscribe();
    }
    self.teardown.push(v);
  }

  fn remove(&mut self, s: &dyn SubscriptionLike) {
    self.teardown.retain(|v| v.inner_addr() != s.inner_addr());
  }
}

impl<T> Default for Inner<T> {
  fn default() -> Self {
    Inner {
      closed: false,
      teardown: SmallVec::new(),
    }
  }
}

macro subscription_direct_impl_proxy() {
  #[inline(always)]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { (&**self).inner_addr() }
}

impl<'a> SubscriptionLike for Box<dyn SubscriptionLike + 'a> {
  subscription_direct_impl_proxy!();
}
impl<'a, Item, Err> SubscriptionLike for Box<dyn Publisher<Item, Err> + 'a> {
  subscription_direct_impl_proxy!();
}

impl SubscriptionLike for Box<dyn SubscriptionLike + Send + Sync> {
  subscription_direct_impl_proxy!();
}
impl<Item, Err> SubscriptionLike
  for Box<dyn Publisher<Item, Err> + Send + Sync>
{
  subscription_direct_impl_proxy!();
}

/// An RAII implementation of a "scoped subscribed" of a subscription.
/// When this structure is dropped (falls out of scope), the subscription will
/// be unsubscribed.
pub struct SubscriptionGuard<T: SubscriptionLike>(pub(crate) T);
impl<T: SubscriptionLike> Drop for SubscriptionGuard<T> {
  #[inline]
  fn drop(&mut self) { self.0.unsubscribe() }
}

subscription_proxy_impl!(SubscriptionGuard<T>, T, 0, <T>);

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn add_remove_for_local() {
    let mut local = LocalSubscription::default();
    let l1 = LocalSubscription::default();
    let l2 = LocalSubscription::default();
    let l3 = LocalSubscription::default();
    local.add(l1.clone());
    assert_eq!(local.0.borrow().teardown.len(), 1);
    local.add(l2.clone());
    assert_eq!(local.0.borrow().teardown.len(), 2);
    local.add(l3.clone());
    assert_eq!(local.0.borrow().teardown.len(), 3);
    local.remove(&l1);
    assert_eq!(local.0.borrow().teardown.len(), 2);
    local.remove(&l2);
    assert_eq!(local.0.borrow().teardown.len(), 1);
    local.remove(&l3);
    assert_eq!(local.0.borrow().teardown.len(), 0);
  }

  #[test]
  fn add_remove_for_shared() {
    let mut local = SharedSubscription::default();
    let l1 = SharedSubscription::default();
    let l2 = SharedSubscription::default();
    let l3 = SharedSubscription::default();
    local.add(l1.clone());
    assert_eq!(local.0.lock().unwrap().teardown.len(), 1);
    local.add(l2.clone());
    assert_eq!(local.0.lock().unwrap().teardown.len(), 2);
    local.add(l3.clone());
    assert_eq!(local.0.lock().unwrap().teardown.len(), 3);
    local.remove(&l1);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 2);
    local.remove(&l2);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 1);
    local.remove(&l3);
    assert_eq!(local.0.lock().unwrap().teardown.len(), 0);
  }
}
