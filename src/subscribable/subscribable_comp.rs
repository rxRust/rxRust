use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeComplete<N, C> {
  next: N,
  complete: C,
}

impl<Item, Err, N, C> Subscribe<Item, Err> for SubscribeComplete<N, C>
where
  N: Fn(&Item),
  C: Fn(),
{
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) {
    match v {
      RxValue::Next(v) => (self.next)(v),
      RxValue::Complete => (self.complete)(),
      _ => {}
    }
  }
}

impl<Item, Err, N, C> IntoSharedSubscribe<Item, Err> for SubscribeComplete<N, C>
where
  N: Fn(&Item) + Send + Sync + 'static,
  C: Fn() + Send + Sync + 'static,
{
  type Shared = Self;
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribableComplete<Item, Err, N, C> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// completion.
  ///
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub;
}

impl<Item, Err, S, N, C> SubscribableComplete<Item, Err, N, C> for S
where
  S: RawSubscribable<Item, Err, SubscribeComplete<N, C>>,
  N: Fn(&Item),
  C: Fn(),
{
  type Unsub = S::Unsub;
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeComplete { next, complete })
  }
}
