use crate::prelude::*;

#[derive(Clone)]
pub struct SkipWhileOp<S, F> {
  pub(crate) source: S,
  pub(crate) predicate: F,
}

impl<Item, Err, O, S, F> Observable<Item, Err, O> for SkipWhileOp<S, F>
where
  S: Observable<Item, Err, SkipWhileObserver<O, F>>,
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(SkipWhileObserver {
      observer,
      predicate: self.predicate,
      done_skipping: false,
    })
  }
}

impl<Item, Err, S, F> ObservableExt<Item, Err> for SkipWhileOp<S, F> where
  S: ObservableExt<Item, Err>
{
}
pub struct SkipWhileObserver<O, F> {
  observer: O,
  predicate: F,
  done_skipping: bool,
}

impl<O, Item, Err, F> Observer<Item, Err> for SkipWhileObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, value: Item) {
    if self.done_skipping {
      self.observer.next(value);
    } else if !(self.predicate)(&value) {
      self.observer.next(value);
      self.done_skipping = true;
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err);
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
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .skip_while(|v| v < &95)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn skip_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip_while5 = observable::from_iter(0..100).skip_while(|v| v < &95);
      let f1 = skip_while5.clone();
      let f2 = skip_while5;

      f1.subscribe(|_| nc1 += 1);
      f2.subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_skip_while);

  fn bench_skip_while(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
