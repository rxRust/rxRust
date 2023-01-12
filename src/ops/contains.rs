use crate::prelude::*;

#[derive(Clone)]
pub struct ContainsOp<S, Item> {
  pub(crate) source: S,
  pub(crate) target: Item,
}

impl<Item, Err, O, S> Observable<bool, Err, O> for ContainsOp<S, Item>
where
  S: Observable<Item, Err, ContainsObserver<O, Item>>,
  O: Observer<bool, Err>,
  Item: PartialEq,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(ContainsObserver {
      observer: Some(observer),
      target: self.target,
    })
  }
}

impl<Item, Err, S> ObservableExt<bool, Err> for ContainsOp<S, Item> where
  S: ObservableExt<Item, Err>
{
}

pub struct ContainsObserver<S, T> {
  observer: Option<S>,
  target: T,
}

impl<O, Item, Err> Observer<Item, Err> for ContainsObserver<O, Item>
where
  O: Observer<bool, Err>,
  Item: PartialEq,
{
  fn next(&mut self, value: Item) {
    if self.target == value {
      if let Some(mut observer) = self.observer.take() {
        observer.next(true);
        observer.complete();
      }
    }
  }

  fn error(mut self, err: Err) {
    if let Some(observer) = self.observer.take() {
      observer.error(err);
    }
  }

  fn complete(mut self) {
    if let Some(mut observer) = self.observer.take() {
      observer.next(false);
      observer.complete();
    }
  }

  fn is_finished(&self) -> bool {
    self.observer.as_ref().map_or(true, |o| o.is_finished())
  }
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
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_contains);

  fn bench_contains(b: &mut bencher::Bencher) {
    b.iter(contains_smoke);
  }
}
