use crate::observer::{
  observer_complete_proxy_impl, observer_error_proxy_impl,
};
use crate::prelude::*;
use ops::SharedOp;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rxrust::{ops::Filter, prelude::*};
///
/// let mut coll = vec![];
/// let coll_clone = coll.clone();
///
/// observable::from_iter(0..10)
///   .filter(|v| *v % 2 == 0)
///   .subscribe(|v| { coll.push(v); });

/// // only even numbers received.
/// assert_eq!(coll, vec![0, 2, 4, 6, 8]);
/// ```

pub trait Filter<T> {
  fn filter<F>(self, filter: F) -> FilterOp<Self, F>
  where
    Self: Sized,
    F: Fn(&T) -> bool,
  {
    FilterOp {
      source: self,
      filter,
    }
  }
}

impl<'a, T, O> Filter<T> for O {}

pub struct FilterOp<S, F> {
  source: S,
  filter: F,
}

impl<O, U, S, F> Observable<O, U> for FilterOp<S, F>
where
  S: Observable<FilterObserver<O, F>, U>,
  U: SubscriptionLike,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let filter = self.filter;
    self.source.actual_subscribe(Subscriber {
      observer: FilterObserver {
        filter,
        observer: subscriber.observer,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct FilterObserver<S, F> {
  observer: S,
  filter: F,
}

impl<Item, O, F> ObserverNext<Item> for FilterObserver<O, F>
where
  O: ObserverNext<Item>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, value: Item) {
    if (self.filter)(&value) {
      self.observer.next(value)
    }
  }
}

observer_error_proxy_impl!(FilterObserver<O, F>, O, observer, <O, F>);
observer_complete_proxy_impl!(FilterObserver<O, F>, O, observer, <O, F>);

impl<S, F> Fork for FilterOp<S, F>
where
  S: Fork,
  F: Clone,
{
  type Output = FilterOp<S::Output, F>;
  fn fork(&self) -> Self::Output {
    FilterOp {
      source: self.source.fork(),
      filter: self.filter.clone(),
    }
  }
}

impl<S, F> IntoShared for FilterOp<S, F>
where
  S: IntoShared,
  F: Send + Sync + 'static,
{
  type Shared = SharedOp<FilterOp<S::Shared, F>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(FilterOp {
      source: self.source.to_shared(),
      filter: self.filter,
    })
  }
}

impl<S, F> IntoShared for FilterObserver<S, F>
where
  S: IntoShared,
  F: Send + Sync + 'static,
{
  type Shared = FilterObserver<S::Shared, F>;
  fn to_shared(self) -> Self::Shared {
    FilterObserver {
      observer: self.observer.to_shared(),
      filter: self.filter,
    }
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Filter, prelude::*};

  #[test]
  fn fork_and_shared() {
    observable::from_iter(0..10)
      .filter(|v| v % 2 == 0)
      .fork()
      .to_shared()
      .filter(|_| true)
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
