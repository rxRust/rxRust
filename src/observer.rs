use crate::subscription::Publisher;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer<Item, Err>:
  ObserverNext<Item> + ObserverError<Err> + ObserverComplete
{
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

impl<Item, Err, T> Observer<Item, Err> for T where
  T: ObserverNext<Item> + ObserverError<Err> + ObserverComplete
{
}

pub(crate) macro observer_next_proxy_impl(
  $t: ty,  $host_ty: ident, $name:tt,
  <$($generics: ident  $(: $bound: tt)?),*>, $item: ident) {
  impl<$($generics $(:$bound)?),*> ObserverNext<$item> for $t
  where
    $host_ty: ObserverNext<$item>,
  {
    #[inline(always)]
    fn next(&mut self, value: $item) { self.$name.next(value); }
  }
}

pub(crate) macro observer_error_proxy_impl(
  $t: ty,  $host_ty: ident, $name:tt,
  <$($generics: ident  $(: $bound: tt)?),*>, $err: ident) {
  impl<$($generics $(:$bound)?),*> ObserverError<$err> for $t
    where $host_ty: ObserverError<$err>
  {
    #[inline(always)]
    fn error(&mut self, err: $err) { self.$name.error(err); }
  }
}

pub(crate) macro observer_complete_proxy_impl(
  $t: ty, $host_ty: ident, $name: tt, <$($generics: tt),*>) {
  impl<$($generics),*> ObserverComplete for $t
    where $host_ty: ObserverComplete
  {
    #[inline(always)]
    fn complete(&mut self) { self.$name.complete(); }
  }
}

macro observer_next_pointer_proxy_impl(
  $item: ty,$ty: ty, <$($generics: tt),*>) {
  impl<$($generics),*> ObserverNext<$item> for $ty {
    #[inline(always)]
    fn next(&mut self, value: $item) { (&mut **self).next(value); }
  }
}

macro observer_error_pointer_proxy_impl(
  $err: ty, $ty: ty, <$($generics: tt),*>) {
  impl<$($generics),*> ObserverError<$err> for $ty {
    #[inline(always)]
    fn error(&mut self, err: $err) { (&mut **self).error(err); }
  }
}

macro observer_complete_pointer_proxy_impl(
  $ty: ty, <$($generics: tt),*>) {
  impl<$($generics),*> ObserverComplete for $ty {
    #[inline(always)]
  fn complete(&mut self) { (&mut **self).complete(); }
  }
}

// implement `Observer` for Box<dyn Observer<Item, Err> + 'a>
observer_next_pointer_proxy_impl!(
  Item, Box<dyn Observer<Item, Err> + 'a>, <'a, Item, Err>);
observer_error_pointer_proxy_impl!(
  Err, Box<dyn Observer<Item, Err> + 'a>, <'a, Item, Err>);
observer_complete_pointer_proxy_impl!(
  Box<dyn Observer<Item, Err> + 'a>, <'a, Item, Err>);

// implement `Observer` for Box<dyn Observer<Item, Err> + Send + Sync>
observer_next_pointer_proxy_impl!(
  Item, Box<dyn Observer<Item, Err> + Send + Sync>, <'a, Item, Err>);
observer_error_pointer_proxy_impl!(
  Err, Box<dyn Observer<Item, Err> + Send + Sync>, <'a, Item, Err>);
observer_complete_pointer_proxy_impl!(
  Box<dyn Observer<Item, Err> + Send + Sync>, <'a, Item, Err>);

// implement `Observer` for Box<dyn Publisher<Item, Err> + 'a>
observer_next_pointer_proxy_impl!(
  Item, Box<dyn Publisher<Item, Err> + 'a>, <'a, Item, Err>);
observer_error_pointer_proxy_impl!(
  Err, Box<dyn Publisher<Item, Err> + 'a>, <'a, Item, Err>);
observer_complete_pointer_proxy_impl!(
  Box<dyn Publisher<Item, Err> + 'a>, <'a, Item, Err>);

// implement `Observer` for Box<dyn Publisher<Item, Err> + Send + Sync>
observer_next_pointer_proxy_impl!(
  Item, Box<dyn Publisher<Item, Err> + Send + Sync>, <'a, Item, Err>);
observer_error_pointer_proxy_impl!(
  Err, Box<dyn Publisher<Item, Err> + Send + Sync>, <'a, Item, Err>);
observer_complete_pointer_proxy_impl!(
  Box<dyn Publisher<Item, Err> + Send + Sync>, <'a, Item, Err>);

impl<Item, S> ObserverNext<Item> for Arc<Mutex<S>>
where
  S: ObserverNext<Item>,
{
  fn next(&mut self, value: Item) { self.lock().unwrap().next(value); }
}

impl<Err, S> ObserverError<Err> for Arc<Mutex<S>>
where
  S: ObserverError<Err>,
{
  fn error(&mut self, err: Err) { self.lock().unwrap().error(err); }
}

impl<S> ObserverComplete for Arc<Mutex<S>>
where
  S: ObserverComplete,
{
  fn complete(&mut self) { self.lock().unwrap().complete(); }
}

impl<Item, S> ObserverNext<Item> for Rc<RefCell<S>>
where
  S: ObserverNext<Item>,
{
  fn next(&mut self, value: Item) { self.borrow_mut().next(value); }
}

impl<Err, S> ObserverError<Err> for Rc<RefCell<S>>
where
  S: ObserverError<Err>,
{
  fn error(&mut self, err: Err) { self.borrow_mut().error(err); }
}

impl<S> ObserverComplete for Rc<RefCell<S>>
where
  S: ObserverComplete,
{
  fn complete(&mut self) { self.borrow_mut().complete(); }
}
