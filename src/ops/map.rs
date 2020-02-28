use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

#[derive(Clone)]
pub struct MapOp<S, M> {
  pub(crate) source: S,
  pub(crate) func: M,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let map = self.func;
    self.source.actual_subscribe(Subscriber {
      observer: MapObserver {
        observer: subscriber.observer,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<Item, S, M> Observable for MapOp<S, M>
where
  S: Observable,
  M: FnMut(S::Item) -> Item,
{
  type Item = Item;
  type Err = S::Err;
}

impl<'a, Item, S, M> LocalObservable<'a> for MapOp<S, M>
where
  S: LocalObservable<'a>,
  M: FnMut(S::Item) -> Item + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription,'a);
}

impl<Item, S, M> SharedObservable for MapOp<S, M>
where
  S: SharedObservable,
  M: FnMut(S::Item) -> Item + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

#[derive(Clone)]
pub struct MapObserver<O, M> {
  observer: O,
  map: M,
}

impl<Item, Err, O, M, B> Observer<Item, Err> for MapObserver<O, M>
where
  O: Observer<B, Err>,
  M: FnMut(Item) -> B,
{
  fn next(&mut self, value: Item) { self.observer.next((self.map)(value)) }
  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn primitive_type() {
    let mut i = 0;
    observable::from_iter(100..101)
      .map(|v| v * 2)
      .subscribe(|v| i += v);
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of(100).map(|v| v).subscribe(|v| i += v);
    assert_eq!(i, 100);
  }

  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).map(|v| v);
    m.map(|v| v).to_shared().subscribe(|_| {});

    // type mapped to other type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).map(|_v| 1);
    m.map(|v| v as f32).to_shared().subscribe(|_| {});

    // ref to ref can fork
    let m = observable::of(&1).map(|v| v);
    m.map(|v| v).to_shared().subscribe(|_| {});
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .map(|_v| 1)
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }
}
