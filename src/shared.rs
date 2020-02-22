use crate::prelude::*;

/// Shared wrap the Observable， subscribe and accept subscribe in a safe mode
/// by SharedObservable.
#[derive(Clone)]
pub struct Shared<R>(pub(crate) R);

pub trait SharedObservable {
  type Item;
  type Err;
  type Unsub: SubscriptionLike + 'static;
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
