use crate::prelude::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct SkipLastOp<S> {
  pub(crate) source: S,
  pub(crate) count: usize,
}

impl<Item, Err, O, S> Observable<Item, Err, O> for SkipLastOp<S>
where
  S: Observable<Item, Err, SkipLastObserver<O, Item>>,
  O: Observer<Item, Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(SkipLastObserver {
      observer,
      count_down: self.count,
      queue: VecDeque::new(),
    })
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for SkipLastOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct SkipLastObserver<O, Item> {
  observer: O,
  count_down: usize,
  queue: VecDeque<Item>,
}

impl<Item, Err, O> Observer<Item, Err> for SkipLastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.queue.push_back(value);
    if self.count_down == 0 {
      self.observer.next(self.queue.pop_front().unwrap());
    } else {
      self.count_down -= 1;
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
    let mut ticks = vec![];

    observable::from_iter(0..10)
      .skip_last(5)
      .on_complete(|| completed = true)
      .subscribe(|v| ticks.push(v));

    assert_eq!(ticks, vec![0, 1, 2, 3, 4]);
    assert!(completed);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut ticks = vec![];

    observable::from_iter(0..10)
      .skip_last(11)
      .on_complete(|| completed = true)
      .subscribe(|v| ticks.push(v));

    assert_eq!(ticks, vec![]);
    assert!(completed);
  }

  #[test]
  fn skip_last_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip_last5 = observable::from_iter(0..100).skip_last(5);
      let f1 = skip_last5.clone();
      let f2 = skip_last5;

      f1.skip_last(5).subscribe(|_| nc1 += 1);
      f2.skip_last(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 90);
    assert_eq!(nc2, 90);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_skip_last);

  fn bench_skip_last(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
