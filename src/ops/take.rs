use crate::prelude::*;

#[derive(Clone)]
pub struct TakeOp<S> {
  source: S,
  count: usize,
}

impl<S> TakeOp<S> {
  pub fn new(source: S, count: usize) -> Self {
    TakeOp { source, count }
  }
}

impl<Item, Err, O, S> Observable<Item, Err, O> for TakeOp<S>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, TakeObserver<O>>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let observer = TakeObserver {
      observer: Some(observer),
      count: self.count,
      hits: 0,
    };
    self.source.actual_subscribe(observer)
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for TakeOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct TakeObserver<O> {
  observer: Option<O>,
  count: usize,
  hits: usize,
}

impl<Item, Err, O> Observer<Item, Err> for TakeObserver<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    if self.hits < self.count {
      if let Some(observer) = self.observer.as_mut() {
        self.hits += 1;
        observer.next(value);
        if self.hits == self.count {
          self.observer.take().unwrap().complete()
        }
      }
    }
  }

  #[inline]
  fn error(mut self, err: Err) {
    if let Some(observer) = self.observer.take() {
      observer.error(err)
    }
  }

  #[inline]
  fn complete(mut self) {
    if let Some(observer) = self.observer.take() {
      observer.complete()
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
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take(5)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn take_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take5 = observable::from_iter(0..100).take(5);
      let f1 = take5.clone();
      let f2 = take5;

      f1.take(5).subscribe(|_| nc1 += 1);
      f2.take(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_take);

  fn bench_take(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
