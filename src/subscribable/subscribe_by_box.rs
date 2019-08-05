use crate::prelude::*;
use crate::subscribable::Subscribable;

pub trait SubscribableByBox {
  type Item;
  type Err;

  fn subscribe_err_complete(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    error: Box<dyn Fn(&Self::Err) + Send + Sync>,
    complete: Box<dyn Fn() + Send + Sync>,
  ) -> Box<dyn Subscription>;

  fn subscribe_err(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    error: Box<dyn Fn(&Self::Err) + Send + Sync>,
  ) -> Box<dyn Subscription>;

  fn subscribe_complete(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
    complete: Box<dyn Fn() + Send + Sync>,
  ) -> Box<dyn Subscription>;

  fn subscribe(
    self,
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
  ) -> Box<dyn Subscription>;
}

impl<S> SubscribableByBox for S
where
  S: Subscribable,
  S::Item: 'static,
  S::Err: 'static,
{
  type Item = S::Item;
  type Err = S::Err;

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
    next: Box<dyn Fn(&Self::Item) + Send + Sync>,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self::Subscribable::subscribe(self, next)
  }
}

#[test]
fn object_safety() {
  let _so: Box<dyn SubscribableByBox<Item = i32, Err = ()>> =
    Box::new(observable::of(1));
}
