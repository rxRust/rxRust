use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribePure<N>(N);

impl<N> ObserverComplete for SubscribePure<N> {
  #[inline(always)]
  fn complete(&mut self) {}
}

impl<N> ObserverError<()> for SubscribePure<N> {
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
}

impl<N, Item> ObserverNext<Item> for SubscribePure<N>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.0)(value); }
}

impl<N> IntoShared for SubscribePure<N>
where
  N: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribablePure<N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> Self::Unsub;
}

impl<S, N> SubscribablePure<N> for S
where
  S: RawSubscribable<Subscriber<SubscribePure<N>, LocalSubscription>>,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(Subscriber::local(SubscribePure(next)))
  }
}
