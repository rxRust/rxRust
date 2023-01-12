use crate::prelude::*;

#[derive(Clone)]
pub struct LastOp<S, Item> {
  pub(crate) source: S,
  pub(crate) last: Option<Item>,
}

impl<S, Item> LastOp<S, Item> {
  #[inline]
  pub(crate) fn new(source: S) -> Self {
    LastOp { source, last: None }
  }
}

impl<Item, S, Err, O> Observable<Item, Err, O> for LastOp<S, Item>
where
  S: Observable<Item, Err, LastObserver<O, Item>>,
  O: Observer<Item, Err>,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(LastObserver { observer, last: self.last })
  }
}

impl<Item, S, Err> ObservableExt<Item, Err> for LastOp<S, Item> where
  S: ObservableExt<Item, Err>
{
}
pub struct LastObserver<S, T> {
  observer: S,
  last: Option<T>,
}

impl<O, Item, Err> Observer<Item, Err> for LastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.last = Some(value);
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(mut self) {
    if let Some(v) = self.last.take() {
      self.observer.next(v)
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
  fn last_or_hundered_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::from_iter(0..100)
      .last_or(200)
      .on_complete(|| completed += 1)
      .on_error(|_| errors += 1)
      .subscribe(|v| last_item = Some(v));

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(99), last_item);
  }

  #[test]
  fn last_or_no_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::empty()
      .last_or(100)
      .on_error(|_| errors += 1)
      .on_complete(|| completed += 1)
      .subscribe(|v| last_item = Some(v));

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(100), last_item);
  }

  #[test]
  fn last_one_item() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::from_iter(0..2)
      .last()
      .on_complete(|| completed += 1)
      .on_error(|_| errors += 1)
      .subscribe(|v| last_item = Some(v));

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(1), last_item);
  }

  #[test]
  fn last_no_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::empty()
      .last()
      .on_error(|_| errors += 1)
      .on_complete(|| completed += 1)
      .subscribe(|v: i32| last_item = Some(v));

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(None, last_item);
  }

  #[test]
  fn last_support_fork() {
    let mut value = 0;
    let mut value2 = 0;
    {
      let o = observable::from_iter(1..100).last();
      let o1 = o.clone().last();
      let o2 = o.last();
      o1.subscribe(|v| value = v);
      o2.subscribe(|v| value2 = v);
    }
    assert_eq!(value, 99);
    assert_eq!(value2, 99);
  }

  #[test]
  fn last_or_support_fork() {
    let mut default = 0;
    let mut default2 = 0;
    let o = observable::create(|subscriber: Subscriber<_>| {
      subscriber.complete();
    })
    .last_or(100);
    let o1 = o.clone().last_or(0);
    let o2 = o.clone().last_or(0);
    let u1: Box<dyn FnMut(i32) + '_> = Box::new(|v| default = v);
    let u2: Box<dyn FnMut(i32) + '_> = Box::new(|v| default2 = v);
    o1.subscribe(u1);
    o2.subscribe(u2);
    assert_eq!(default, 100);
    assert_eq!(default, 100);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_last);

  fn bench_last(b: &mut bencher::Bencher) {
    b.iter(last_or_hundered_items);
  }
}
