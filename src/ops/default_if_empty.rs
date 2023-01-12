use crate::prelude::*;

#[derive(Clone)]
pub struct DefaultIfEmptyOp<S, Item> {
  source: S,
  is_empty: bool,
  default_value: Item,
}

impl<Item, S> DefaultIfEmptyOp<S, Item> {
  pub(crate) fn new(source: S, default_value: Item) -> Self {
    Self { source, is_empty: true, default_value }
  }
}

impl<Item, Err, O, S> Observable<Item, Err, O> for DefaultIfEmptyOp<S, Item>
where
  S: Observable<Item, Err, DefaultIfEmptyObserver<O, Item>>,
  O: Observer<Item, Err>,
  Item: Clone,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(DefaultIfEmptyObserver {
      observer,
      is_empty: self.is_empty,
      default_value: self.default_value,
    })
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for DefaultIfEmptyOp<S, Item> where
  S: ObservableExt<Item, Err>
{
}
pub struct DefaultIfEmptyObserver<O, Item> {
  observer: O,
  is_empty: bool,
  default_value: Item,
}

impl<Item, Err, O> Observer<Item, Err> for DefaultIfEmptyObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: Clone,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    if self.is_empty {
      self.is_empty = false;
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  fn complete(mut self) {
    if self.is_empty {
      self.observer.next(self.default_value.clone());
    }
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
  use bencher::Bencher;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut value = 0;

    observable::of(10)
      .default_if_empty(5)
      .on_complete(|| completed = true)
      .subscribe(|v| value = v);

    assert_eq!(value, 10);
    assert!(completed);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut value = 0;

    observable::empty()
      .default_if_empty(5)
      .on_complete(|| completed = true)
      .subscribe(|v| value = v);

    assert_eq!(value, 5);
    assert!(completed);
  }

  #[test]
  fn bench_base() {
    bench_b();
  }

  benchmark_group!(bench_b, bench_base_function);

  fn bench_base_function(b: &mut Bencher) {
    b.iter(base_function);
  }

  #[test]
  fn bench_empty() {
    bench_e();
  }

  benchmark_group!(bench_e, bench_empty_function);

  fn bench_empty_function(b: &mut Bencher) {
    b.iter(base_empty_function);
  }
}
