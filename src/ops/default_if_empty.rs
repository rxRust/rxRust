use crate::error_proxy_impl;
use crate::prelude::*;

#[derive(Clone)]
pub struct DefaultIfEmptyOp<S>
where
  S: Observable,
{
  pub(crate) source: S,
  pub(crate) is_empty: bool,
  pub(crate) default_value: S::Item,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let subscriber = Subscriber {
      observer: DefaultIfEmptyObserver {
        observer: subscriber.observer,
        is_empty: self.is_empty,
        default_value: self.default_value,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}
}

observable_proxy_impl!(DefaultIfEmptyOp, S);

impl<'a, S> LocalObservable<'a> for DefaultIfEmptyOp<S>
where
  S: LocalObservable<'a>,
  S::Item: Clone + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S> SharedObservable for DefaultIfEmptyOp<S>
where
  S: SharedObservable,
  S::Item: Clone + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct DefaultIfEmptyObserver<O, Item> {
  observer: O,
  is_empty: bool,
  default_value: Item,
}

impl<Item, Err, O> Observer for DefaultIfEmptyObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    if self.is_empty {
      self.is_empty = false;
    }
  }

  fn complete(&mut self) {
    if self.is_empty {
      self.observer.next(self.default_value.clone());
    }
    self.observer.complete()
  }

  error_proxy_impl!(Err, observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use bencher::Bencher;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut value = 0;

    observable::of(10)
      .default_if_empty(5)
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 10);
    assert!(completed);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut value = 0;

    observable::empty()
      .default_if_empty(5)
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 5);
    assert!(completed);
  }

  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .default_if_empty(5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn ininto_shared_empty() {
    observable::empty()
      .default_if_empty(5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench_base() { bench_b(); }

  benchmark_group!(bench_b, bench_base_funciton);

  fn bench_base_funciton(b: &mut Bencher) { b.iter(base_function); }

  #[test]
  fn bench_empty() { bench_e(); }

  benchmark_group!(bench_e, bench_empty_funciton);

  fn bench_empty_funciton(b: &mut Bencher) { b.iter(base_empty_function); }
}
