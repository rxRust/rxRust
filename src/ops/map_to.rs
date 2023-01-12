use crate::prelude::*;

#[derive(Clone)]
pub struct MapToOp<S, B, Item> {
  source: S,
  value: B,
  _m: TypeHint<Item>,
}

impl<S, B, Item> MapToOp<S, B, Item> {
  #[inline]
  pub fn new(source: S, value: B) -> Self {
    Self { source, value, _m: TypeHint::default() }
  }
}
impl<Item, Err, O, S, B> Observable<B, Err, O> for MapToOp<S, B, Item>
where
  S: Observable<Item, Err, MapToObserver<O, B>>,
  O: Observer<B, Err>,
  B: Clone,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(MapToObserver { observer, value: self.value })
  }
}

impl<Item, Err, S, B> ObservableExt<B, Err> for MapToOp<S, B, Item> where
  S: ObservableExt<Item, Err>
{
}

#[derive(Clone)]
pub struct MapToObserver<O, B> {
  observer: O,
  value: B,
}

impl<Item, Err, O, B> Observer<Item, Err> for MapToObserver<O, B>
where
  O: Observer<B, Err>,
  B: Clone,
{
  #[inline]
  fn next(&mut self, _: Item) {
    self.observer.next(self.value.clone())
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
      .map_to(5)
      .subscribe(|v| i += v);
    assert_eq!(i, 5);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of(100).map_to(5).subscribe(|v| i += v);
    assert_eq!(i, 5);
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .map_to(1)
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_map_to);

  fn bench_map_to(b: &mut bencher::Bencher) {
    b.iter(primitive_type);
  }
}
