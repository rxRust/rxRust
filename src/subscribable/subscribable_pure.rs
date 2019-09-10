use crate::prelude::*;

#[repr(transparent)]
pub struct SubscribePure<N>(N);

impl<Item, Err, N> Subscribe<Item, Err> for SubscribePure<N>
where
  N: Fn(&Item),
{
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) {
    if let RxValue::Next(v) = v {
      (self.0)(v);
    }
  }
}
pub trait SubscribablePure<Item, Err, N> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> Self::Unsub;
}

impl<Item, Err, S, N> SubscribablePure<Item, Err, N> for S
where
  S: RawSubscribable<Item, Err, SubscribePure<N>>,
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
