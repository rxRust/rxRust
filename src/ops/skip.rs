use crate::prelude::*;

#[derive(Clone)]
pub struct SkipOp<S> {
  source: S,
  count: usize,
}

impl<S> SkipOp<S> {
  #[inline]
  pub fn new(source: S, count: usize) -> Self {
    Self { source, count }
  }
}

impl<S, Item, Err, O> Observable<Item, Err, O> for SkipOp<S>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, SkipObserver<O>>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(SkipObserver {
      observer,
      count: self.count,
      hits: 0,
    })
  }
}

impl<S, Item, Err> ObservableExt<Item, Err> for SkipOp<S> where
  S: ObservableExt<Item, Err>
{
}
pub struct SkipObserver<O> {
  observer: O,
  count: usize,
  hits: usize,
}

impl<Item, Err, O> Observer<Item, Err> for SkipObserver<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.hits += 1;
    if self.hits > self.count {
      self.observer.next(value);
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
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .skip(5)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 95);
    assert!(completed);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .skip(101)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 0);
    assert!(completed);
  }

  #[test]
  fn skip_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip5 = observable::from_iter(0..100).skip(5);
      let f1 = skip5.clone();
      let f2 = skip5;

      f1.skip(5).subscribe(|_| nc1 += 1);
      f2.skip(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 90);
    assert_eq!(nc2, 90);
  }

  #[test]
  fn benchmark() {
    do_bench();
  }

  benchmark_group!(do_bench, bench);

  fn bench(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
