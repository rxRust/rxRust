use crate::prelude::*;

#[derive(Clone)]
pub struct FilterOp<S, F> {
  pub(crate) source: S,
  pub(crate) filter: F,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $source:ident, $($marker:ident +)* $lf: lifetime) => {
  type Unsub = $source::Unsub;
  fn actual_subscribe<O>(
    self,
    observer:O,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let filter = self.filter;
    self.source.actual_subscribe(FilterObserver {
      filter,
      observer
    })
  }
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

impl<Item, Err, O, F> Observer for FilterObserver<O, F>
where
  O: Observer<Item = Item, Err = Err>,
  F: FnMut(&Item) -> bool,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if (self.filter)(&value) {
      self.observer.next(value)
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
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
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn smoke() {
    observable::from_iter(0..1000)
      .filter(|v| v % 2 == 0)
      .subscribe(|v| {
        assert!(v % 2 == 0);
      });
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_filter);

  fn bench_filter(b: &mut bencher::Bencher) { b.iter(smoke); }
}
