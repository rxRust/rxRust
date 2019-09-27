use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribePure<N>(N);

impl<Item, N> Subscribe<Item, ()> for SubscribePure<N>
where
  N: FnMut(&Item),
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { (self.0)(value); }
  #[inline(always)]
  fn on_error(&mut self, _err: &()) {}
  #[inline(always)]
  fn on_complete(&mut self) {}
}

impl<N> IntoShared for SubscribePure<N>
where
  N: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribablePure<Item, N> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> Self::Unsub;
}

impl<Item, S, N> SubscribablePure<Item, N> for S
where
  S: RawSubscribable<Item, (), Subscriber<SubscribePure<N>, LocalSubscription>>,
  N: FnMut(&Item),
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(Subscriber::new(SubscribePure(next)))
  }
}
