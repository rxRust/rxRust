use crate::prelude::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct TakeLastOp<S> {
  pub(crate) source: S,
  pub(crate) count: usize,
}

impl<Item, Err, O, S> Observable<Item, Err, O> for TakeLastOp<S>
where
  S: Observable<Item, Err, TakeLastObserver<O, Item>>,
  O: Observer<Item, Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(TakeLastObserver {
      observer,
      count: self.count,
      queue: VecDeque::new(),
    })
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for TakeLastOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct TakeLastObserver<O, Item> {
  observer: O,
  count: usize,
  queue: VecDeque<Item>, // TODO: replace VecDeque with RingBuf
}

impl<Item, Err, O> Observer<Item, Err> for TakeLastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.queue.push_back(value);
    while self.queue.len() > self.count {
      self.queue.pop_front();
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  fn complete(mut self) {
    for value in self.queue.drain(..) {
      self.observer.next(value);
    }
    self.observer.complete();
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

    observable::from_iter(0..100)
      .take_last(5)
      .on_complete(|| completed = true)
      .subscribe(|v| ticks.push(v));

    assert_eq!(ticks, vec![95, 96, 97, 98, 99]);
    assert!(completed);
  }

  #[test]
  fn take_last_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take_last5 = observable::from_iter(0..100).take_last(5);
      let f1 = take_last5.clone();
      let f2 = take_last5;

      f1.take_last(5).subscribe(|_| nc1 += 1);
      f2.take_last(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_take_last);

  fn bench_take_last(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
