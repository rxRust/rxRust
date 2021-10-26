use crate::error_proxy_impl;
use crate::prelude::*;

#[derive(Clone)]
pub struct ContainsOp<S, Item> {
  pub(crate) source: S,
  pub(crate) target: Item,
}

impl<S, Item> Observable for ContainsOp<S, Item>
where
  S: Observable<Item = Item>,
{
  type Item = bool;
  type Err = S::Err;
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=bool,Err= Self::Err> + $($marker +)* $lf {
    let subscriber = Subscriber {
      observer: ContainsObserver{
        observer: subscriber.observer,
        target: self.target,
        done:false,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}
}

impl<'a, Item, S> LocalObservable<'a> for ContainsOp<S, Item>
where
  S: LocalObservable<'a, Item = Item>,
  Item: 'a + Clone + Eq,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription,'a);
}

impl<Item, S> SharedObservable for ContainsOp<S, Item>
where
  S: SharedObservable<Item = Item>,
  Item: Send + Sync + 'static + Clone + Eq,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct ContainsObserver<S, T> {
  observer: S,
  target: T,
  done: bool,
}

impl<O, Item, Err> Observer for ContainsObserver<O, Item>
where
  O: Observer<Item = bool, Err = Err>,
  Item: Clone + Eq,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if !self.done && self.target == value {
      self.observer.next(true);
      self.done = true;
      self.observer.complete();
    }
  }

  fn complete(&mut self) {
    if !self.done {
      self.observer.next(false);
      self.observer.complete();
    }
  }

  error_proxy_impl!(Err, observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  #[test]
  fn contains_smoke() {
    observable::from_iter(0..10)
      .contains(4)
      .subscribe(|b| assert!(b));
    observable::from_iter(0..10)
      .contains(99)
      .subscribe(|b| assert!(!b));
    observable::empty().contains(1).subscribe(|b| assert!(!b));
  }

  #[test]
  fn contains_shared() {
    observable::from_iter(0..10)
      .contains(4)
      .into_shared()
      .subscribe(|b| assert!(b));
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_contains);

  fn bench_contains(b: &mut bencher::Bencher) { b.iter(contains_smoke); }
}
