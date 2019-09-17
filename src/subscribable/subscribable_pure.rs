use crate::prelude::*;

#[repr(transparent)]
#[derive(Clone)]
pub struct SubscribePure<N>(N);

impl<Item, N> Subscribe<Item, ()> for SubscribePure<N>
where
  N: Fn(&Item),
{
  fn run(&self, v: RxValue<&'_ Item, &'_ ()>) {
    if let RxValue::Next(v) = v {
      (self.0)(v);
    }
  }
}

impl<Item, N> IntoSharedSubscribe<Item, ()> for SubscribePure<N>
where
  N: Fn(&Item) + Send + Sync + 'static,
{
  type Shared = Self;
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
  S: RawSubscribable<Item, (), SubscribePure<N>>,
  N: Fn(&Item),
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribePure(next))
  }
}
