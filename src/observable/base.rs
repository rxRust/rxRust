use crate::prelude::*;

pub trait Emitter<'a> {
  type Item;
  type Err;
  fn emit<O, U>(self, subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err> + 'a,
    U: SubscriptionLike + 'a;
}

#[derive(Clone)]
pub struct ObservableBase<Emit>(Emit);

impl<Emit> ObservableBase<Emit> {
  pub fn new(emitter: Emit) -> Self { ObservableBase(emitter) }
}

impl<'a, Emit> Observable<'a> for ObservableBase<Emit>
where
  Emit: Emitter<'a>,
{
  type Item = Emit::Item;
  type Err = Emit::Err;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + 'a,
    U: SubscriptionLike + Clone + 'static,
  >(
    self,
    subscriber: Subscriber<O, U>,
  ) -> U {
    let subscription = subscriber.subscription.clone();
    self.0.emit(subscriber);
    subscription
  }
}

impl<Emit> SharedObservable for ObservableBase<Emit>
where
  Emit: SharedEmitter + Send + Sync + 'static,
{
  type Item = Emit::Item;
  type Err = Emit::Err;
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    let subscription = subscriber.subscription.clone();
    self.0.shared_emit(subscriber);
    subscription
  }
}

impl<Emit> Fork for ObservableBase<Emit>
where
  Self: Clone,
{
  type Output = Self;
  #[inline]
  fn fork(&self) -> Self::Output { self.clone() }
}
