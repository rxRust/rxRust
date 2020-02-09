use crate::prelude::*;
use ops::SharedOp;

pub trait Emitter {
  type Item;
  type Err;
  fn emit<O, U>(self, subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err>,
    U: SubscriptionLike;
}

pub trait SharedEmitter {
  type Item;
  type Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static;
}

impl<T> IntoShared for T
where
  T: Emitter + Sync + Send + 'static,
{
  type Shared = SharedOp<Self>;
  fn to_shared(self) -> Self::Shared { SharedOp(self) }
}

impl<T> SharedEmitter for SharedOp<T>
where
  T: Emitter + Send + Sync + 'static,
{
  type Item = T::Item;
  type Err = T::Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    self.0.emit(subscriber)
  }
}

#[derive(Clone)]
pub struct ObservableBase<Emit>(Emit);

impl<Emit> ObservableBase<Emit> {
  pub fn new(emitter: Emit) -> Self { ObservableBase(emitter) }
}

impl<Emit, O, U> Observable<O, U> for ObservableBase<Emit>
where
  O: Observer<Emit::Item, Emit::Err>,
  U: SubscriptionLike + Clone,
  Emit: Emitter,
{
  type Unsub = U;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.0.emit(subscriber);
    subscription
  }
}

impl<Emit, O, U> Observable<O, U> for ObservableBase<SharedOp<Emit>>
where
  Emit: Send + Sync + 'static,
  O: IntoShared,
  O::Shared: Observer<
      <SharedOp<Emit> as SharedEmitter>::Item,
      <SharedOp<Emit> as SharedEmitter>::Err,
    > + Send
    + Sync
    + 'static,
  U: IntoShared<Shared = SharedSubscription> + SubscriptionLike,
  SharedOp<Emit>: SharedEmitter,
{
  type Unsub = U::Shared;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let subscription = subscriber.subscription.clone();
    self.0.shared_emit(subscriber);
    subscription
  }
}

impl<Emit> IntoShared for ObservableBase<Emit>
where
  Emit: IntoShared,
  Self: Send + Sync + 'static,
{
  type Shared = ObservableBase<Emit::Shared>;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { ObservableBase(self.0.to_shared()) }
}

impl<Emit> Fork for ObservableBase<Emit>
where
  Self: Clone,
{
  type Output = Self;
  #[inline]
  fn fork(&self) -> Self::Output { self.clone() }
}
