use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeComplete<N, C> {
  next: N,
  complete: C,
}

impl<N, C> ObserverComplete for SubscribeComplete<N, C>
where
  C: FnMut(),
{
  #[inline(always)]
  fn complete(&mut self) { (self.complete)(); }
}

impl<N, C> ObserverError<()> for SubscribeComplete<N, C> {
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
}

impl<N, C, Item> ObserverNext<Item> for SubscribeComplete<N, C>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
}

impl<N, C> SubscribeComplete<N, C> {
  #[inline(always)]
  pub fn new(next: N, complete: C) -> Self {
    SubscribeComplete { next, complete }
  }
}

impl<N, C> IntoShared for SubscribeComplete<N, C>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribableComplete<N, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub;
}

impl<S, N, C> SubscribableComplete<N, C> for S
where
  S: RawSubscribable<Subscriber<SubscribeComplete<N, C>, LocalSubscription>>,
{
  type Unsub = S::Unsub;
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(Subscriber::local(SubscribeComplete { next, complete }))
  }
}
