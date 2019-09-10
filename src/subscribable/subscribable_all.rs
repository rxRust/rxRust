use crate::prelude::*;

pub struct SubscribeAll<N, E, C> {
  next: N,
  error: E,
  complete: C,
}

impl<Item, Err, N, E, C> Subscribe<Item, Err> for SubscribeAll<N, E, C>
where
  N: Fn(&Item),
  E: Fn(&Err),
  C: Fn(),
{
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) {
    match v {
      RxValue::Next(v) => (self.next)(v),
      RxValue::Err(e) => (self.error)(e),
      RxValue::Complete => (self.complete)(),
    };
  }
}
pub trait SubscribableAll<Item, Err, N, E, C> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  ///
  fn subscribe_all(self, next: N, error: E, complete: C) -> Self::Unsub;
}

impl<S, Item, Err, N, E, C> SubscribableAll<Item, Err, N, E, C> for S
where
  S: RawSubscribable<Item, Err, SubscribeAll<N, E, C>>,
  N: Fn(&Item),
  E: Fn(&Err),
  C: Fn(),
{
  type Unsub = S::Unsub;
  fn subscribe_all(self, next: N, error: E, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeAll {
      next,
      error,
      complete,
    })
  }
}
