mod subscribable_all;
pub use subscribable_all::*;
mod subscribable_err;
pub use subscribable_err::*;
mod subscribable_pure;
pub use subscribable_pure::*;
mod subscribable_comp;
use crate::subscription::SubscriptionLike;
use std::sync::{Arc, Mutex};
pub use subscribable_comp::*;

use std::cell::RefCell;
use std::rc::Rc;

/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer<Item, Err> {
  fn next(&mut self, value: &Item);
  fn error(&mut self, err: &Err);
  fn complete(&mut self);
}

pub trait IntoShared {
  type Shared: Sync + Send + 'static;
  fn to_shared(self) -> Self::Shared;
}

pub trait RawSubscribable<Item, Err, Subscriber> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike + 'static;
  fn raw_subscribe(self, subscriber: Subscriber) -> Self::Unsub;
}

impl<'a, Item, Err> Observer<Item, Err> for Box<dyn Observer<Item, Err> + 'a> {
  #[inline(always)]
  fn next(&mut self, value: &Item) { (&mut **self).next(value); }
  #[inline(always)]
  fn error(&mut self, err: &Err) { (&mut **self).error(err); }
  #[inline(always)]
  fn complete(&mut self) { (&mut **self).complete(); }
}

impl<Item, Err> Observer<Item, Err>
  for Box<dyn Observer<Item, Err> + Send + Sync>
{
  #[inline(always)]
  fn next(&mut self, value: &Item) { (&mut **self).next(value); }
  #[inline(always)]
  fn error(&mut self, err: &Err) { (&mut **self).error(err); }
  #[inline(always)]
  fn complete(&mut self) { (&mut **self).complete(); }
}

impl<Item, Err, S> Observer<Item, Err> for Arc<Mutex<S>>
where
  S: Observer<Item, Err>,
{
  fn next(&mut self, value: &Item) { self.lock().unwrap().next(value); }
  fn error(&mut self, err: &Err) { self.lock().unwrap().error(err); }
  fn complete(&mut self) { self.lock().unwrap().complete(); }
}

impl<Item, Err, S> Observer<Item, Err> for Rc<RefCell<S>>
where
  S: Observer<Item, Err>,
{
  fn next(&mut self, value: &Item) { self.borrow_mut().next(value); }
  fn error(&mut self, err: &Err) { self.borrow_mut().error(err); }
  fn complete(&mut self) { self.borrow_mut().complete(); }
}

impl<Item, Err> IntoShared for Box<dyn Observer<Item, Err> + Send + Sync>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}
impl<S> IntoShared for Arc<Mutex<S>>
where
  S: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}
// todo: define a safe RawSubscribable return a Box<Subscription> let
// we can crate a object safety object ref.
