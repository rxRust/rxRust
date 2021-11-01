use crate::prelude::*;

#[derive(Clone)]
pub struct FilterMapOp<S, F> {
  pub(crate) source: S,
  pub(crate) f: F,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    observer: O,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    self.source.actual_subscribe(FilterMapObserver {
      down_observer: observer,
      f: self.f,
      _marker: TypeHint::new(),
    })
  }
}
}

impl<'a, Item, S, F> Observable for FilterMapOp<S, F>
where
  S: Observable,
  F: FnMut(S::Item) -> Option<Item>,
{
  type Item = Item;
  type Err = S::Err;
}

impl<'a, Item, S, F> LocalObservable<'a> for FilterMapOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut(S::Item) -> Option<Item> + 'a,
  S::Item: 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<Item, S, F> SharedObservable for FilterMapOp<S, F>
where
  S: SharedObservable,
  F: FnMut(S::Item) -> Option<Item> + Send + Sync + 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct FilterMapObserver<O, F, Item> {
  down_observer: O,
  f: F,
  _marker: TypeHint<*const Item>,
}

impl<O, F, Item, Err, OutputItem> Observer for FilterMapObserver<O, F, Item>
where
  O: Observer<Item = OutputItem, Err = Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if let Some(v) = (self.f)(value) {
      self.down_observer.next(v)
    }
  }

  fn error(&mut self, err: Self::Err) { self.down_observer.error(err) }

  fn complete(&mut self) { self.down_observer.complete() }
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
  fn filter_map_shared_and_fork() {
    observable::of(1)
      .filter_map(|_| Some("str"))
      .clone()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn filter_map_return_ref() {
    observable::of(&1)
      .filter_map(Some)
      .clone()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_map_types_mixed);

  fn bench_map_types_mixed(b: &mut bencher::Bencher) {
    b.iter(map_types_mixed);
  }
}
