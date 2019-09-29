use crate::prelude::*;
use std::any::Any;
use std::cell::RefCell;
use std::mem::replace;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait SubscriptionLike {
  /// This allows deregistering an stream before it has finished receiving all
  /// events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);

  fn is_closed(&self) -> bool;
}

enum Teardown<T> {
  None,
  Once(T),
  Multi(Vec<T>),
}

macro inner_add($inner: expr, $v: expr) {
  if $inner.closed {
    $v.unsubscribe();
  }
  let teardown = &mut $inner.teardown;
  match teardown {
    Teardown::None => *teardown = Teardown::Once($v),
    Teardown::Once(_) => {
      let first = replace(teardown, Teardown::None);
      if let Teardown::Once(first) = first {
        *teardown = Teardown::Multi(vec![first, $v])
      }
    }
    Teardown::Multi(ref mut vec) => vec.push($v),
  }
}

macro inner_unsubscribe($inner: expr) {
  if !$inner.closed {
    $inner.closed = true;
    match $inner.teardown {
      Teardown::None => {}
      Teardown::Once(ref mut first) => first.unsubscribe(),
      Teardown::Multi(ref mut vec) => {
        vec.iter_mut().for_each(|v| v.unsubscribe())
      }
    }
  }
}

struct Inner<T> {
  closed: bool,
  teardown: Teardown<T>,
}

impl<T> Default for Inner<T> {
  fn default() -> Self {
    Inner {
      closed: false,
      teardown: Teardown::None,
    }
  }
}

#[derive(Clone, Default)]
pub struct LocalSubscription(Rc<RefCell<Inner<Box<dyn SubscriptionLike>>>>);

impl LocalSubscription {
  pub fn add<S: SubscriptionLike + Any + 'static>(&mut self, subscription: S) {
    let of_any = &subscription as &dyn Any;
    if let Some(local) = of_any.downcast_ref::<LocalSubscription>() {
      if Rc::ptr_eq(&self.0, &local.0) {
        return;
      }
    }
    let mut s = Box::new(subscription);
    inner_add!(self.0.borrow_mut(), s);
  }
}

impl IntoShared for LocalSubscription {
  type Shared = SharedSubscription;
  fn to_shared(self) -> SharedSubscription {
    let inner = self.0.borrow();
    match inner.teardown {
      Teardown::None => SharedSubscription(Arc::new(Mutex::new(Inner {
        closed: inner.closed,
        teardown: Teardown::None,
      }))),
      _ => panic!(
        "LocalSubscription already has some teardown work to do,
         can not covert to SharedSubscription "
      ),
    }
  }
}

impl SubscriptionLike for LocalSubscription {
  fn unsubscribe(&mut self) {
    let mut inner = self.0.borrow_mut();
    inner_unsubscribe!(inner);
  }

  fn is_closed(&self) -> bool { self.0.borrow_mut().closed }
}

#[derive(Clone, Default)]
pub struct SharedSubscription(
  Arc<Mutex<Inner<Box<dyn SubscriptionLike + Send + Sync>>>>,
);

impl SharedSubscription {
  pub fn add<S: SubscriptionLike + Any + Send + Sync + 'static>(
    &mut self,
    subscription: S,
  ) {
    let of_any = &subscription as &dyn Any;
    if let Some(local) = of_any.downcast_ref::<SharedSubscription>() {
      if Arc::ptr_eq(&self.0, &local.0) {
        return;
      }
    }

    let inner = &mut *self.0.lock().unwrap();
    let mut s = Box::new(subscription);
    inner_add!(inner, s);
  }
}

impl SubscriptionLike for SharedSubscription {
  fn unsubscribe(&mut self) {
    let mut unsub = self.0.lock().unwrap();
    inner_unsubscribe!(unsub);
  }

  fn is_closed(&self) -> bool { self.0.lock().unwrap().closed }
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
