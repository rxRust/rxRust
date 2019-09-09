use crate::prelude::*;
use std::marker::PhantomData;

pub struct SubscribePure<Item, Err, N> {
  next: N,
  _ph: PhantomData<(Item, Err)>,
}

impl<Item, Err, N> Subscribe for SubscribePure<Item, Err, N>
where
  N: Fn(&Item),
{
  type Item = Item;
  type Err = Err;
  fn run(&self, v: RxValue<&'_ Self::Item, &'_ Self::Err>) {
    if let RxValue::Next(v) = v {
      (self.next)(v);
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

impl<S, Item, Err, N> SubscribablePure<Item, Err, N> for S
where
  S: RawSubscribable<SubscribePure<Item, Err, N>>,
  N: Fn(&Item),
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribePure {
      next,
      _ph: PhantomData,
    })
  }
}
