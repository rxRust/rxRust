use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

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

#[derive(Clone)]
pub struct FilterOp<S, F> {
  source: S,
  filter: F,
}

macro observable_impl(
  $subscription:ty, $source:ident, $($marker:ident +)* $lf: lifetime)
{
  type Item = $source::Item;
  type Err = $source::Err;
  type Unsub = $source::Unsub;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
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

impl<'a, S, F> Observable<'a> for FilterOp<S, F>
where
  S: Observable<'a>,
  F: FnMut(&S::Item) -> bool + 'a,
{
  observable_impl!(LocalSubscription, S, 'a);
}

impl<S, F> SharedObservable for FilterOp<S, F>
where
  S: SharedObservable,
  F: FnMut(&S::Item) -> bool + Send + Sync + 'static,
{
  observable_impl!(SharedSubscription, S, Send + Sync + 'static);
}

pub struct FilterObserver<S, F> {
  observer: S,
  filter: F,
}

impl<Item, Err, O, F> Observer<Item, Err> for FilterObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, value: Item) {
    if (self.filter)(&value) {
      self.observer.next(value)
    }
  }
  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
}

#[cfg(test)]
mod test {
  use crate::{ops::Filter, prelude::*};

  #[test]
  fn fork_and_shared() {
    observable::from_iter(0..10)
      .filter(|v| v % 2 == 0)
      .clone()
      .filter(|_| true)
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }
}
