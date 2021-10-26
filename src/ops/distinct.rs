use crate::prelude::*;
use crate::{complete_proxy_impl, error_proxy_impl};
use std::{cmp::Eq, collections::HashSet, hash::Hash};

#[derive(Clone)]
pub struct DistinctOp<S> {
  pub(crate) source: S,
}

observable_proxy_impl!(DistinctOp, S);

macro_rules! distinct_impl {
  ( $subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let subscriber = Subscriber {
      observer: DistinctObserver {
        observer: subscriber.observer,
        seen: HashSet::new(),
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}
}

impl<'a, S, Item> LocalObservable<'a> for DistinctOp<S>
where
  S: LocalObservable<'a, Item = Item>,
  Item: 'a + Eq + Hash + Clone,
{
  type Unsub = S::Unsub;
  distinct_impl!(LocalSubscription,'a);
}

impl<S, Item> SharedObservable for DistinctOp<S>
where
  S: SharedObservable<Item = Item>,
  Item: Hash + Eq + Clone + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  distinct_impl!(SharedSubscription, Send + Sync + 'static);
}

struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

impl<O, Item, Err> Observer for DistinctObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Hash + Eq + Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if !self.seen.contains(&value) {
      self.seen.insert(value.clone());
      self.observer.next(value);
    }
  }
  complete_proxy_impl!(observer);
  error_proxy_impl!(Err, observer);
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(0..20)
      .map(|v| v % 5)
      .distinct()
      .subscribe(move |v| x.borrow_mut().push(v))
      .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[0, 1, 2, 3, 4]);
  }
  #[test]
  fn shared() {
    observable::from_iter(0..10)
      .distinct()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }
  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_distinct);

  fn bench_distinct(b: &mut bencher::Bencher) { b.iter(smoke); }
}
