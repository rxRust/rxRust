use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeComplete<N, C> {
  next: N,
  complete: C,
}

impl<Item, N, C> Subscribe<Item, ()> for SubscribeComplete<N, C>
where
  N: FnMut(&Item),
  C: FnMut(),
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { (self.next)(value); }
  #[inline(always)]
  fn on_error(&mut self, _err: &()) {}
  #[inline(always)]
  fn on_complete(&mut self) { (self.complete)(); }
}

impl<N, C> IntoShared for SubscribeComplete<N, C>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribableComplete<Item, N, C> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub;
}

impl<Item, S, N, C> SubscribableComplete<Item, N, C> for S
where
  S: RawSubscribable<Item, (), SubscribeComplete<N, C>>,
  N: FnMut(&Item),
  C: FnMut(),
{
  type Unsub = S::Unsub;
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeComplete { next, complete })
  }
}
