use crate::prelude::*;

#[derive(Clone)]
pub struct TapOp<S, M> {
  pub(crate) source: S,
  pub(crate) func: M,
}

impl<Item, Err, S, M, O> Observable<Item, Err, O> for TapOp<S, M>
where
  S: Observable<Item, Err, TapObserver<O, M>>,
  M: FnMut(&Item),
  O: Observer<Item, Err>,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let func = self.func;
    self.source.actual_subscribe(TapObserver { observer, func })
  }
}

impl<Item, Err, S, M> ObservableExt<Item, Err> for TapOp<S, M> where
  S: ObservableExt<Item, Err>
{
}
#[derive(Clone)]
pub struct TapObserver<O, F> {
  observer: O,
  func: F,
}

impl<Item, Err, O, F> Observer<Item, Err> for TapObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item),
{
  fn next(&mut self, value: Item) {
    (self.func)(&value);
    self.observer.next(value)
  }

  fn error(self, err: Err) {
    self.observer.error(err)
  }

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
  fn primitive_type() {
    let mut i = 0;
    let mut v = 0;
    observable::from_iter(100..101)
      .tap(|i| v = *i)
      .subscribe(|v| i += v);
    assert_eq!(i, 100);
    assert_eq!(v, 100);
  }

  #[test]
  fn benchmark() {
    do_bench();
  }

  benchmark_group!(do_bench, bench);

  fn bench(b: &mut bencher::Bencher) {
    b.iter(primitive_type);
  }
}
