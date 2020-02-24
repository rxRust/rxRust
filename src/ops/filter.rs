use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

#[derive(Clone)]
pub struct FilterOp<S, F> {
  pub(crate) source: S,
  pub(crate) filter: F,
}

macro observable_impl(
  $subscription:ty, $source:ident, $($marker:ident +)* $lf: lifetime)
{
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

impl<S, F> Observable for FilterOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl<'a, S, F> LocalObservable<'a> for FilterOp<S, F>
where
  S: LocalObservable<'a>,
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
  use crate::prelude::*;

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
