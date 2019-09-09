use crate::prelude::*;
use std::marker::PhantomData;

pub struct SubscribeAll<Item, Err, N, E, C> {
  next: N,
  error: E,
  complete: C,
  _ph: PhantomData<(Item, Err)>,
}

impl<Item, Err, N, E, C> Subscribe for SubscribeAll<Item, Err, N, E, C>
where
  N: Fn(&Item),
  E: Fn(&Err),
  C: Fn(),
{
  type Item = Item;
  type Err = Err;
  fn run(&self, v: RxValue<&'_ Self::Item, &'_ Self::Err>) {
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
  S: RawSubscribable<SubscribeAll<Item, Err, N, E, C>>,
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
      _ph: PhantomData,
    })
  }
}
