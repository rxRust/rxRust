use crate::prelude::*;

pub trait SubscribableByFnPtr: Subscribable + Sized
where
  Self::Item: 'static,
  Self::Err: 'static,
{
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
