mod subscribable_all;
pub use subscribable_all::*;
mod subscribable_err;
pub use subscribable_err::*;
mod subscribable_pure;
pub use subscribable_pure::*;
mod subscribable_comp;
use crate::subscription::{Publisher, SubscriptionLike};
use std::sync::{Arc, Mutex};
pub use subscribable_comp::*;

use std::cell::RefCell;
use std::rc::Rc;

/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer<Item, Err> {
  fn next(&mut self, value: Item);
  fn error(&mut self, err: Err);
  fn complete(&mut self);
}

/// ObserverNext can consume the items delivered by an Observable.
pub trait ObserverNext<Item> {
  fn next(&mut self, value: Item);
}

/// ObserverNext can consume the error delivered by an Observable.
pub trait ObserverError<Err> {
  fn error(&mut self, err: Err);
}

/// ObserverNext can consume the complete notify delivered by an Observable.
pub trait ObserverComplete {
  fn complete(&mut self);
}

impl<Item, Err, T> Observer<Item, Err> for T
where
  T: ObserverNext<Item> + ObserverError<Err> + ObserverComplete,
{
  #[inline(always)]
  fn next(&mut self, value: Item) { self.next(value); }
  #[inline(always)]
  fn error(&mut self, err: Err) { self.error(err); }
  #[inline(always)]
  fn complete(&mut self) { self.complete() }
}

pub trait IntoShared {
  type Shared: Sync + Send + 'static;
  fn to_shared(self) -> Self::Shared;
}

pub trait RawSubscribable<Subscriber> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike + 'static;
  fn raw_subscribe(self, subscriber: Subscriber) -> Self::Unsub;
}

macro observer_impl_proxy_directly($item: ty, $err: ty) {
  #[inline(always)]
  fn next(&mut self, value: $item) { (&mut **self).next(value); }
  #[inline(always)]
  fn error(&mut self, err: $err) { (&mut **self).error(err); }
  #[inline(always)]
  fn complete(&mut self) { (&mut **self).complete(); }
}

impl<'a, Item, Err> Observer<Item, Err> for Box<dyn Observer<Item, Err> + 'a> {
  observer_impl_proxy_directly!(Item, Err);
}

impl<'a, Item, Err> Observer<Item, Err> for Box<dyn Publisher<Item, Err> + 'a> {
  observer_impl_proxy_directly!(Item, Err);
}

impl<'a, Item, Err> Observer<&mut Item, Err>
  for Box<dyn for<'r> Publisher<&'r mut Item, Err> + 'a>
{
  observer_impl_proxy_directly!(&mut Item, Err);
}
impl<'a, Item, Err> Observer<Item, &mut Err>
  for Box<dyn for<'r> Publisher<Item, &'r mut Err> + 'a>
{
  observer_impl_proxy_directly!(Item, &mut Err);
}
impl<'a, Item, Err> Observer<&mut Item, &mut Err>
  for Box<dyn for<'r> Publisher<&'r mut Item, &'r mut Err> + 'a>
{
  observer_impl_proxy_directly!(&mut Item, &mut Err);
}
impl<Item, Err> Observer<Item, Err>
  for Box<dyn Observer<Item, Err> + Send + Sync>
{
  observer_impl_proxy_directly!(Item, Err);
}
impl<Item, Err> Observer<Item, Err>
  for Box<dyn Publisher<Item, Err> + Send + Sync>
{
  observer_impl_proxy_directly!(Item, Err);
}

impl<Item, Err, S> Observer<Item, Err> for Arc<Mutex<S>>
where
  S: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.lock().unwrap().next(value); }
  fn error(&mut self, err: Err) { self.lock().unwrap().error(err); }
  fn complete(&mut self) { self.lock().unwrap().complete(); }
}

impl<Item, Err, S> Observer<Item, Err> for Rc<RefCell<S>>
where
  S: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.borrow_mut().next(value); }
  fn error(&mut self, err: Err) { self.borrow_mut().error(err); }
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
