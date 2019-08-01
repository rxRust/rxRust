use crate::prelude::*;

pub trait SubscribableByBox: Subscribable + Sized
where
  Self::Item: 'static,
  Self::Err: 'static,
{
  fn subscribe_err_complete(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    error: Box<dyn Fn(&Self::Err) + Send + Sync>,
    complete: Box<dyn Fn() + Send + Sync>,
  ) -> Box<dyn Subscription> {
    self::Subscribable::subscribe_err_complete(self, next, error, complete)
  }

  fn subscribe_err(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    error: Box<dyn Fn(&Self::Err) + Send + Sync>,
  ) -> Box<dyn Subscription> {
    self::Subscribable::subscribe_err(self, next, error)
  }

  fn subscribe_complete(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    complete: Box<dyn Fn() + Send + Sync>,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self::Subscribable::subscribe_complete(self, next, complete)
  }

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self::Subscribable::subscribe(self, next)
  }
}
