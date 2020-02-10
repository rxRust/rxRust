use crate::prelude::*;

#[derive(Clone)]
pub struct Shared<R: Send + Sync + 'static>(pub(crate) R);

pub trait SharedObservable {
  type Item;
  type Err;
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription;
}

pub trait SharedEmitter {
  type Item;
  type Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static;
}

impl<E> SharedEmitter for Shared<E>
where
  E: SharedEmitter + Send + Sync + 'static,
{
  type Item = E::Item;
  type Err = E::Err;
  #[inline(always)]
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<E::Item, E::Err> + Send + Sync + 'static,
  {
    self.0.shared_emit(subscriber)
  }
}

impl<T> SharedObservable for Shared<T>
where
  T: SharedObservable + Send + Sync + 'static,
{
  type Item = T::Item;
  type Err = T::Err;
  #[inline(always)]
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    self.0.shared_actual_subscribe(subscriber)
  }
}
