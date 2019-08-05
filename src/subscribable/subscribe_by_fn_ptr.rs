use crate::prelude::*;
use crate::subscribable::Subscribable;

pub trait SubscribableByFnPtr {
  type Item;
  type Err;
  fn subscribe_err_complete(
    self,
    next: fn(&Self::Item),
    error: fn(&Self::Err),
    complete: fn(),
  ) -> Box<dyn Subscription>;

  fn subscribe_err(
    self,
    next: fn(&Self::Item),
    error: fn(&Self::Err),
  ) -> Box<dyn Subscription>;

  fn subscribe_complete(
    self,
    next: fn(&Self::Item),
    complete: fn(),
  ) -> Box<dyn Subscription>;

  fn subscribe(self, next: fn(&Self::Item)) -> Box<dyn Subscription>;
}

impl<S> SubscribableByFnPtr for S
where
  S: Subscribable,
  S::Item: 'static,
  S::Err: 'static,
{
  type Item = S::Item;
  type Err = S::Err;

  #[inline(always)]
  fn subscribe_err_complete(
    self,
    next: fn(&Self::Item),
    error: fn(&Self::Err),
    complete: fn(),
  ) -> Box<dyn Subscription> {
    self::Subscribable::subscribe_err_complete(self, next, error, complete)
  }

  #[inline(always)]
  fn subscribe_err(
    self,
    next: fn(&Self::Item),
    error: fn(&Self::Err),
  ) -> Box<dyn Subscription> {
    self::Subscribable::subscribe_err(self, next, error)
  }

  #[inline(always)]
  fn subscribe_complete(
    self,
    next: fn(&Self::Item),
    complete: fn(),
  ) -> Box<dyn Subscription> {
    self::Subscribable::subscribe_complete(self, next, complete)
  }

  #[inline(always)]
  fn subscribe(self, next: fn(&Self::Item)) -> Box<dyn Subscription> {
    self::Subscribable::subscribe(self, next)
  }
}

#[test]
fn object_safety() {
  let _so: Box<dyn SubscribableByFnPtr<Item = i32, Err = ()>> =
    Box::new(observable::of(1));
}
