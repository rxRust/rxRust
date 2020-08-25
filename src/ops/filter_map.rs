use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

#[derive(Clone)]
pub struct FilterMapOp<S, F> {
  pub(crate) source: S,
  pub(crate) f: F,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: FilterMapObserver {
        down_observer: subscriber.observer,
        f: self.f,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<'a, Item, S, F> Observable for FilterMapOp<S, F>
where
  S: Observable,
  F: FnMut(S::Item) -> Option<Item>,
{
  type Item = Item;
  type Err = S::Err;
}

impl<'a, Item, S, F> LocalObservable<'a> for FilterMapOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut(S::Item) -> Option<Item> + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<Item, S, F> SharedObservable for FilterMapOp<S, F>
where
  S: SharedObservable,
  F: FnMut(S::Item) -> Option<Item> + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct FilterMapObserver<O, F> {
  down_observer: O,
  f: F,
}

impl<O, F, Item, Err, OutputItem> Observer<Item, Err>
  for FilterMapObserver<O, F>
where
  O: Observer<OutputItem, Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  fn next(&mut self, value: Item) {
    if let Some(v) = (self.f)(value) {
      self.down_observer.next(v)
    }
  }
  error_proxy_impl!(Err, down_observer);
  complete_proxy_impl!(down_observer);

  #[inline]
  fn is_stopped(&self) -> bool { self.down_observer.is_stopped() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .filter_map(|_v| Some(1))
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }

  #[test]
  fn filter_map_shared_and_fork() {
    observable::of(1)
      .filter_map(|_| Some("str"))
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn filter_map_return_ref() {
    observable::of(&1)
      .filter_map(Some)
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }
}
