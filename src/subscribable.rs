mod subscribable_all;
pub use subscribable_all::*;
mod subscribable_err;
pub use subscribable_err::*;
mod subscribable_pure;
pub use subscribable_pure::*;
mod subscribable_comp;
pub use subscribable_comp::*;

/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Subscribe<Item, Err> {
  fn on_next(&mut self, value: &Item);
  fn on_error(&mut self, err: &Err);
  fn on_complete(&mut self);
}

pub trait IntoShared {
  type Shared: Sync + Send + 'static;
  fn to_shared(self) -> Self::Shared;
}

pub trait RawSubscribable<Item, Err, Subscribe> {
  /// a type implemented [`Subscription`]
  type Unsub;
  fn raw_subscribe(self, subscribe: Subscribe) -> Self::Unsub;
}

impl<'a, Item, Err> Subscribe<Item, Err>
  for Box<dyn Subscribe<Item, Err> + 'a>
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { (&mut **self).on_next(value); }
  #[inline(always)]
  fn on_error(&mut self, err: &Err) { (&mut **self).on_error(err); }
  #[inline(always)]
  fn on_complete(&mut self) { (&mut **self).on_complete(); }
}

impl<Item, Err> Subscribe<Item, Err>
  for Box<dyn Subscribe<Item, Err> + Send + Sync>
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { (&mut **self).on_next(value); }
  #[inline(always)]
  fn on_error(&mut self, err: &Err) { (&mut **self).on_error(err); }
  #[inline(always)]
  fn on_complete(&mut self) { (&mut **self).on_complete(); }
}

impl<Item, Err> IntoShared for Box<dyn Subscribe<Item, Err> + Send + Sync>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

// todo: define a safe RawSubscribable return a Box<Subscription> let
// we can crate a object safety object ref.
