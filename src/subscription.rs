use crate::prelude::*;
use crate::util;
use std::cell::RefCell;
use std::mem::replace;
use std::mem::transmute;
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

impl LocalSubscription {
  pub fn add<S: SubscriptionLike + 'static>(&mut self, subscription: S) {
    if self.inner_addr() != subscription.inner_addr() {
      self.0.borrow_mut().add(Box::new(subscription))
    }
  }

  pub fn remove(&mut self, subscription: &dyn SubscriptionLike) {
    self.0.borrow_mut().remove(subscription);
  }

  pub fn teardown_size(&self) -> usize { self.0.borrow().teardown.size() }
}

impl IntoShared for LocalSubscription {
  type Shared = SharedSubscription;
  fn to_shared(self) -> SharedSubscription {
    let inner = util::unwrap_rc_ref_cell(
      self.0,
      "Cannot convert a `LocalSubscription` to `SharedSubscription` \
       when it referenced by other.",
    );

    match inner.teardown {
      Teardown::None => {}
      _ => panic!(
        "LocalSubscription already has some teardown work to do,
         can not covert to SharedSubscription "
      ),
    };
    let inner: Inner<Box<dyn SubscriptionLike + Send + Sync + 'static>> =
      unsafe { transmute(inner) };
    SharedSubscription(Arc::new(Mutex::new(Inner {
      teardown: inner.teardown,
      closed: inner.closed,
    })))
  }
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

  pub fn teardown_size(&self) -> usize {
    self.0.lock().unwrap().teardown.size()
  }
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

impl IntoShared for SharedSubscription {
  type Shared = SharedSubscription;
  #[inline(always)]
  fn to_shared(self) -> SharedSubscription { self }
}

pub trait Publisher<Item, Err>: Observer<Item, Err> + SubscriptionLike {}

impl<Item, Err, T> Publisher<Item, Err> for T where
  T: Observer<Item, Err> + SubscriptionLike
{
}

enum Teardown<T> {
  None,
  Once(T),
  Multi(Vec<T>),
}

impl<T> Teardown<T> {
  fn size(&self) -> usize {
    match self {
      Teardown::None => 0,
      Teardown::Once(_) => 1,
      Teardown::Multi(ref vec) => vec.len(),
    }
  }
}

struct Inner<T> {
  closed: bool,
  teardown: Teardown<T>,
}

impl<T: SubscriptionLike> Inner<T> {
  #[inline(always)]
  fn is_closed(&self) -> bool { self.closed }

  fn unsubscribe(&mut self) {
    if !self.closed {
      self.closed = true;
      match self.teardown {
        Teardown::None => {}
        Teardown::Once(ref mut first) => first.unsubscribe(),
        Teardown::Multi(ref mut vec) => {
          vec.iter_mut().for_each(|v| v.unsubscribe())
        }
      }
    }
  }

  fn add(&mut self, mut v: T) {
    if self.closed {
      v.unsubscribe();
    }
    let teardown = &mut self.teardown;
    match teardown {
      Teardown::None => *teardown = Teardown::Once(v),
      Teardown::Once(_) => {
        let first = replace(teardown, Teardown::None);
        if let Teardown::Once(first) = first {
          *teardown = Teardown::Multi(vec![first, v])
        }
      }
      Teardown::Multi(ref mut vec) => vec.push(v),
    }
  }
  fn remove(&mut self, s: &dyn SubscriptionLike) {
    let teardown = &mut self.teardown;
    match teardown {
      Teardown::None => {}
      Teardown::Once(ref mut first) => {
        if first.inner_addr() == s.inner_addr() {
          replace(teardown, Teardown::None);
        }
      }
      Teardown::Multi(ref mut vec) => {
        vec.retain(|v| v.inner_addr() != s.inner_addr());
        if vec.len() == 1 {
          let once = Teardown::Once(vec.pop().unwrap());
          replace(teardown, once);
        }
      }
    }
  }
}

impl<T> Default for Inner<T> {
  fn default() -> Self {
    Inner {
      closed: false,
      teardown: Teardown::None,
    }
  }
}

impl<'a> SubscriptionLike for Box<dyn SubscriptionLike + 'a> {
  #[inline(always)]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { (&**self).inner_addr() }
}

impl SubscriptionLike for Box<dyn SubscriptionLike + Send + Sync> {
  #[inline(always)]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { (&**self).inner_addr() }
}

impl<'a, Item, Err> SubscriptionLike for Box<dyn Publisher<Item, Err> + 'a> {
  #[inline(always)]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { (&**self).inner_addr() }
}

impl<Item, Err> SubscriptionLike
  for Box<dyn Publisher<Item, Err> + Send + Sync>
{
  #[inline(always)]
  fn unsubscribe(&mut self) { (&mut **self).unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { (&**self).is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { (&**self).inner_addr() }
}

#[cfg(test)]
mod test {
  use super::*;
  fn teardown_size<T>(td: &Teardown<T>) -> usize {
    match td {
      Teardown::None => 0,
      Teardown::Once(_) => 1,
      Teardown::Multi(ref vec) => vec.len(),
    }
  }
  #[test]
  fn add_remove_for_local() {
    let mut local = LocalSubscription::default();
    let l1 = LocalSubscription::default();
    let l2 = LocalSubscription::default();
    let l3 = LocalSubscription::default();
    local.add(l1.clone());
    assert_eq!(teardown_size(&local.0.borrow().teardown), 1);
    local.add(l2.clone());
    assert_eq!(teardown_size(&local.0.borrow().teardown), 2);
    local.add(l3.clone());
    assert_eq!(teardown_size(&local.0.borrow().teardown), 3);
    local.remove(&l1);
    assert_eq!(teardown_size(&local.0.borrow().teardown), 2);
    local.remove(&l2);
    assert_eq!(teardown_size(&local.0.borrow().teardown), 1);
    local.remove(&l3);
    assert_eq!(teardown_size(&local.0.borrow().teardown), 0);
  }

  #[test]
  fn add_remove_for_shared() {
    let mut local = SharedSubscription::default();
    let l1 = SharedSubscription::default();
    let l2 = SharedSubscription::default();
    let l3 = SharedSubscription::default();
    local.add(l1.clone());
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 1);
    local.add(l2.clone());
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 2);
    local.add(l3.clone());
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 3);
    local.remove(&l1);
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 2);
    local.remove(&l2);
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 1);
    local.remove(&l3);
    assert_eq!(teardown_size(&local.0.lock().unwrap().teardown), 0);
  }
}
