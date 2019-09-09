use crate::prelude::*;
use std::marker::PhantomData;

pub struct SubscribeErr<Item, Err, N, E> {
  next: N,
  error: E,
  _ph: PhantomData<(Item, Err)>,
}

impl<Item, Err, N, E> Subscribe for SubscribeErr<Item, Err, N, E>
where
  N: Fn(&Item),
  E: Fn(&Err),
{
  type Item = Item;
  type Err = Err;
  fn run(&self, v: RxValue<&'_ Self::Item, &'_ Self::Err>) {
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

impl<S, Item, Err, N, E> SubscribableErr<Item, Err, N, E> for S
where
  S: RawSubscribable<SubscribeErr<Item, Err, N, E>>,
  N: Fn(&Item),
  E: Fn(&Err),
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeErr {
      next,
      error,
      _ph: PhantomData,
    })
  }
}
