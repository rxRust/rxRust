use crate::prelude::*;
use std::marker::PhantomData;

pub struct SubscribeComplete<Item, Err, N, C> {
  next: N,
  complete: C,
  _ph: PhantomData<(Item, Err)>,
}

impl<Item, Err, N, C> Subscribe for SubscribeComplete<Item, Err, N, C>
where
  N: Fn(&Item),
  C: Fn(),
{
  type Item = Item;
  type Err = Err;
  fn run(&self, v: RxValue<&'_ Self::Item, &'_ Self::Err>) {
    match v {
      RxValue::Next(v) => (self.next)(v),
      RxValue::Complete => (self.complete)(),
      _ => {}
    }
  }
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

impl<S, Item, Err, N, C> SubscribableComplete<Item, Err, N, C> for S
where
  S: RawSubscribable<SubscribeComplete<Item, Err, N, C>>,
  N: Fn(&Item),
  C: Fn(),
{
  type Unsub = S::Unsub;
  fn subscribe_complete(self, next: N, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeComplete {
      next,
      complete,
      _ph: PhantomData,
    })
  }
}
