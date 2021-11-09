use crate::prelude::*;

/// Shared wrap the Observable， subscribe and accept subscribe in a safe mode
/// by SharedObservable.
#[derive(Clone)]
pub struct Shared<R>(pub(crate) R);

pub trait SharedObservable: Observable {
  type Unsub: SubscriptionLike + Sync + Send + 'static;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static;

  /// Convert to a thread-safe mode.
  #[inline]
  fn into_shared(self) -> Shared<Self>
  where
    Self: Sized,
  {
    Shared(self)
  }
}

impl<S: Observable> Observable for Shared<S> {
  type Item = S::Item;
  type Err = S::Err;
}

impl<S> SharedObservable for Shared<S>
where
  S: SharedObservable,
{
  type Unsub = S::Unsub;
  #[inline]
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self.0.actual_subscribe(observer)
  }
}
