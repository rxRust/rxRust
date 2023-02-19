use crate::prelude::*;

#[derive(Clone)]
pub struct OnErrorMapOp<S, M, Err> {
  source: S,
  func: M,
  _m: TypeHint<Err>,
}

impl<S, M, Err> OnErrorMapOp<S, M, Err> {
  #[inline]
  pub fn new(source: S, func: M) -> Self {
    Self { source, func, _m: TypeHint::default() }
  }
}

impl<Item, Err, OutputErr, O, S, M> Observable<Item, OutputErr, O>
  for OnErrorMapOp<S, M, Err>
where
  S: Observable<Item, Err, OnErrorMapObserver<O, M>>,
  O: Observer<Item, OutputErr>,
  M: FnMut(Err) -> OutputErr,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(OnErrorMapObserver { observer, map: self.func })
  }
}

impl<Item, Err, OutputErr, S, M> ObservableExt<Item, OutputErr>
  for OnErrorMapOp<S, M, Err>
where
  S: ObservableExt<Item, Err>,
  M: FnMut(Err) -> OutputErr,
{
}

#[derive(Clone)]
pub struct OnErrorMapObserver<O, M> {
  observer: O,
  map: M,
}

impl<Item, Err, O, M, B> Observer<Item, Err> for OnErrorMapObserver<O, M>
where
  M: FnMut(Err) -> B,
  O: Observer<Item, B>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next(value)
  }

  #[inline]
  fn error(mut self, err: Err) {
    self.observer.error((self.map)(err))
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
    let _ = create(|mut subscriber: Subscriber<_>| {
      subscriber.next(());
      subscriber.error("Hello");
    })
    .on_error_map(|_| "Test")
    .on_error(|error| assert_eq!(error, "Test"))
    .subscribe(|_| {});
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    create(|mut subscriber: Subscriber<_>| {
      subscriber.next(());
      subscriber.error(100);
    })
    .on_error_map(|x| x)
    .on_error(|v| i += v)
    .subscribe(|_| {});
    assert_eq!(i, 100);
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(());
      subscriber.error("a");
    })
    .on_error_map(|_v| 1)
    .on_error(|v| i += v)
    .subscribe(|_| ());
    assert_eq!(i, 1);
  }

  #[test]
  fn map_to_void() {
    create(|mut publisher: Subscriber<_>| {
      publisher.next("Hello");
      publisher.error("World");
    })
    .on_error(|_| ())
    .subscribe(|_| {});
  }

  #[test]
  fn map_flat_map_from_iter() {
    create(|mut publisher: Subscriber<_>| {
      publisher.next("Hello");
      publisher.error("World");
    })
    .flat_map(|_| from_iter(vec!['a', 'b', 'c']).on_error_map(|_| "World"))
    .on_error(|_| {})
    .subscribe(|_| {});
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
