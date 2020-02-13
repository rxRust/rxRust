use crate::prelude::*;
use observable::SharedConnectableObservable;

/// Shared wrap the Observable， subscribe and accept subscribe in a safe mode by
/// SharedObservable.
#[derive(Clone)]
pub struct Shared<R>(pub(crate) R);

pub trait SharedObservable {
  type Item;
  type Err;
  type Unsub: SubscriptionLike;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub;

  /// Convert to a thread-safe mode.
  #[inline]
  fn to_shared(self) -> Shared<Self>
  where
    Self: Sized,
  {
    Shared(self)
  }

  /// Returns a ConnectableObservable. A ConnectableObservable Observable
  /// resembles an ordinary Observable, except that it does not begin emitting
  /// items when it is subscribed to, but only when the Connect operator is
  /// applied to it. In this way you can wait for all intended observers to
  /// subscribe to the Observable before the Observable begins emitting items.
  ///
  #[inline(always)]
  fn publish(self) -> SharedConnectableObservable<Self, Self::Item, Self::Err>
  where
    Self: Sized,
  {
    SharedConnectableObservable::shared(self)
  }
}

pub trait SharedEmitter {
  type Item;
  type Err;
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static;
}

impl<S> SharedObservable for Shared<S>
where
  S: SharedObservable,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  #[inline]
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    self.0.actual_subscribe(subscriber)
  }
}
