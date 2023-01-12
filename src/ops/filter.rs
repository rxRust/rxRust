use crate::prelude::*;

#[derive(Clone)]
pub struct FilterOp<S, F> {
  pub(crate) source: S,
  pub(crate) filter: F,
}

impl<Item, Err, O, S, F> Observable<Item, Err, O> for FilterOp<S, F>
where
  S: Observable<Item, Err, FilterObserver<O, F>>,
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(FilterObserver { filter: self.filter, observer })
  }
}

impl<Item, Err, S, F> ObservableExt<Item, Err> for FilterOp<S, F> where
  S: ObservableExt<Item, Err>
{
}

pub struct FilterObserver<O, F> {
  observer: O,
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

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn smoke() {
    observable::from_iter(0..1000)
      .filter(|v| v % 2 == 0)
      .subscribe(|v| {
        assert!(v % 2 == 0);
      });
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_filter);

  fn bench_filter(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }
}
