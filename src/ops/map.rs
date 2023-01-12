use crate::prelude::*;

#[derive(Clone)]
pub struct MapOp<S, F, Item> {
  source: S,
  func: F,
  _m: TypeHint<Item>,
}

impl<S, F, Item> MapOp<S, F, Item> {
  #[inline]
  pub fn new(source: S, func: F) -> Self {
    Self { source, func, _m: TypeHint::new() }
  }
}

impl<Item1, Item2, Err, O, S, F> Observable<Item1, Err, O>
  for MapOp<S, F, Item2>
where
  O: Observer<Item1, Err>,
  S: Observable<Item2, Err, MapObserver<O, F>>,
  F: FnMut(Item2) -> Item1,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(MapObserver { observer, map: self.func })
  }
}

impl<Item1, Item2, Err, S, F> ObservableExt<Item1, Err> for MapOp<S, F, Item2>
where
  S: ObservableExt<Item2, Err>,
  F: FnMut(Item2) -> Item1,
{
}

#[derive(Clone)]
pub struct MapObserver<O, F> {
  observer: O,
  map: F,
}

impl<Item, Err, O, F, B> Observer<Item, Err> for MapObserver<O, F>
where
  O: Observer<B, Err>,
  F: FnMut(Item) -> B,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next((self.map)(value))
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
  fn primitive_type() {
    let mut i = 0;
    observable::from_iter(100..101)
      .map(|v| v * 2)
      .subscribe(|v| i += v);
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of(100).map(|v| v).subscribe(|v| i += v);
    assert_eq!(i, 100);
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .map(|_v| 1)
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
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
