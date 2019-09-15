use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeErr<N, E> {
  next: N,
  error: E,
}

impl<Item, Err, N, E> Subscribe<Item, Err> for SubscribeErr<N, E>
where
  N: Fn(&Item),
  E: Fn(&Err),
{
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) {
    match v {
      RxValue::Next(v) => (self.next)(v),
      RxValue::Err(e) => (self.error)(e),
      RxValue::Complete => {}
    };
  }
}
pub trait SubscribableErr<Item, Err, N, E> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// completion.
  ///
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub;
}

impl<Item, Err, S, N, E> SubscribableErr<Item, Err, N, E> for S
where
  S: RawSubscribable<Item, Err, SubscribeErr<N, E>>,
  N: Fn(&Item),
  E: Fn(&Err),
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeErr { next, error })
  }
}
