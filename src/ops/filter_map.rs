use crate::prelude::*;

#[derive(Clone)]
pub struct FilterMapOp<S, F, Item> {
  source: S,
  f: F,
  _m: TypeHint<Item>,
}

impl<S, F, Item> FilterMapOp<S, F, Item> {
  #[inline]
  pub fn new(source: S, f: F) -> Self {
    Self { source, f, _m: TypeHint::default() }
  }
}

impl<OutputItem, Item, Err, O, S, F> Observable<OutputItem, Err, O>
  for FilterMapOp<S, F, Item>
where
  S: Observable<Item, Err, FilterMapObserver<O, F>>,
  O: Observer<OutputItem, Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(FilterMapObserver {
      down_observer: observer,
      f: self.f,
    })
  }
}

impl<OutputItem, Item, Err, S, F> ObservableExt<OutputItem, Err>
  for FilterMapOp<S, F, Item>
where
  S: ObservableExt<Item, Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
}

pub struct FilterMapObserver<O, F> {
  down_observer: O,
  f: F,
}

impl<O, F, Item, Err, OutputItem> Observer<Item, Err>
  for FilterMapObserver<O, F>
where
  O: Observer<OutputItem, Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  fn next(&mut self, value: Item) {
    if let Some(v) = (self.f)(value) {
      self.down_observer.next(v)
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.down_observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.down_observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.down_observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .filter_map(|_v| Some(1))
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }

  #[test]
  fn filter_map_return_ref() {
    observable::of(&1)
      .filter_map(Some)
      .clone()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_map_types_mixed);

  fn bench_map_types_mixed(b: &mut bencher::Bencher) {
    b.iter(map_types_mixed);
  }
}
