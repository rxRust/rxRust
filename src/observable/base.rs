use crate::prelude::*;

pub trait Emitter {
  type Item;
  type Err;
}

pub trait LocalEmitter<'a>: Emitter {
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a;
}

#[derive(Clone)]
pub struct ObservableBase<Emit>(Emit);

impl<Emit> ObservableBase<Emit> {
  pub fn new(emitter: Emit) -> Self { ObservableBase(emitter) }
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let subscription = subscriber.subscription.clone();
    self.0.emit(subscriber);
    subscription
  }
}
}

impl<Emit> Observable for ObservableBase<Emit>
where
  Emit: Emitter,
{
  type Item = Emit::Item;
  type Err = Emit::Err;
}

impl<'a, Emit> LocalObservable<'a> for ObservableBase<Emit>
where
  Emit: LocalEmitter<'a>,
{
  type Unsub = LocalSubscription;
  observable_impl!(LocalSubscription, 'a);
}

impl<Emit> SharedObservable for ObservableBase<Emit>
where
  Emit: SharedEmitter,
{
  type Unsub = SharedSubscription;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}
