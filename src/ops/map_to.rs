use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

#[derive(Clone)]
pub struct MapToOp<S, B> {
  pub(crate) source: S,
  pub(crate) value: B,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let value = self.value;
    self.source.actual_subscribe(Subscriber {
      observer: MapToObserver {
        observer: subscriber.observer,
        value,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<S, B> Observable for MapToOp<S, B>
where
  S: Observable,
{
  type Item = B;
  type Err = S::Err;
}

impl<'a, B, S> LocalObservable<'a> for MapToOp<S, B>
where
  S: LocalObservable<'a>,
  B: PayloadCopy + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription,'a);
}

impl<B, S> SharedObservable for MapToOp<S, B>
where
  S: SharedObservable,
  B: PayloadCopy + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

#[derive(Clone)]
pub struct MapToObserver<O, B> {
  observer: O,
  value: B,
}

impl<Item, Err, O, B> Observer<Item, Err> for MapToObserver<O, B>
where
  O: Observer<B, Err>,
  B: PayloadCopy,
{
  fn next(&mut self, value: Item) {
    self.observer.next(self.value.payload_copy())
  }
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
      .map_to(5)
      .subscribe(|v| i += v);
    assert_eq!(i, 5);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of(100).map_to(5).subscribe(|v| i += v);
    assert_eq!(i, 5);
  }

  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).map_to(5);
    m.map_to(6).to_shared().subscribe(|_| {});

    // type mapped to other type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).map_to(1);
    m.map_to(2.0).to_shared().subscribe(|_| {});

    // ref to ref can fork
    let m = observable::of(&1).map_to(3);
    m.map_to(4).to_shared().subscribe(|_| {});
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .map_to(1)
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }
}
