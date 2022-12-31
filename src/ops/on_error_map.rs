use crate::{impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct OnErrorMapOp<S, M> {
  pub(crate) source: S,
  pub(crate) func: M,
}

impl<Err, S, M> Observable for OnErrorMapOp<S, M>
where
  S: Observable,
  M: FnMut(S::Err) -> Err,
{
  type Item = S::Item;
  type Err = Err;
}

impl_local_shared_both! {
  impl<Err, S, M>  OnErrorMapOp<S, M>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let map = $self.func;
    $self.source.actual_subscribe(OnErrorMapObserver {
      observer: $observer,
      map,
      _marker: TypeHint::new(),
    })
  }
  where
    S: @ctx::Observable,
    S::Err: @ctx::local_only('o) @ctx::shared_only('static),
    M: FnMut(S::Err) -> Err
      + @ctx::local_only('o) @ctx::shared_only( Send + Sync + 'static)
}

#[derive(Clone)]
pub struct OnErrorMapObserver<O, M, Err> {
  observer: O,
  map: M,
  _marker: TypeHint<*const Err>,
}

impl<Item, Err, O, M, B> Observer for OnErrorMapObserver<O, M, Err>
where
  M: FnMut(Err) -> B,
  O: Observer<Item = Item, Err = B>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    self.observer.next(value)
  }

  fn error(&mut self, err: Err) {
    self.observer.error((self.map)(err))
  }

  fn complete(&mut self) {
    self.observer.complete()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn primitive_type() {
    create(|subscribe| {
      subscribe.next(());
      subscribe.error("Hello");
      subscribe.complete();
    })
    .on_error_map(|_| "Test")
    .subscribe_err(|_| {}, |error| assert_eq!(error, "Test"));
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    create(|subscribe| {
      subscribe.next(());
      subscribe.error(100);
      subscribe.complete();
    })
    .on_error_map(|x| x)
    .subscribe_err(|_| {}, |v| i += v);
    assert_eq!(i, 100);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = create(|subscribe| {
      subscribe.next(());
      subscribe.error("Hello World");
      subscribe.complete();
    })
    .on_error_map(|v| v);
    m.on_error_map(|v| v)
      .into_shared()
      .subscribe_err(|_| {}, |e| assert_eq!(e, "Hello World"));
    //
    // type mapped to other type can fork
    let m = create(|subscribe| {
      subscribe.next(());
      subscribe.error("Hello World");
      subscribe.complete();
    })
    .on_error_map(|_| 1);
    m.on_error_map(|v| v as f32)
      .into_shared()
      .subscribe_err(|_| {}, |e| assert_eq!(e, 1f32));

    // ref to ref can fork
    let m = create(|subscribe| {
      subscribe.next(());
      subscribe.error(&100);
      subscribe.complete();
    })
    .on_error_map(|v| v);
    m.on_error_map(|v| v)
      .into_shared()
      .subscribe_err(|_| {}, |_| {});
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::create(|subscribe| {
      subscribe.next(());
      subscribe.error("a");
      subscribe.complete();
    })
    .on_error_map(|_v| 1)
    .subscribe_err(|_| (), |v| i += v);
    assert_eq!(i, 1);
  }

  #[test]
  fn map_to_void() {
    create(|subscribe| {
      subscribe.next("Hello");
      subscribe.error("World");
      subscribe.complete();
    })
    .on_error_map(|_| ())
    .subscribe(|_| {});
  }

  #[test]
  fn map_flat_map_from_iter() {
    create(|subscribe| {
      subscribe.next("Hello");
      subscribe.error("World");
      subscribe.complete();
    })
    .flat_map(|_| from_iter(vec!['a', 'b', 'c']).on_error_map(|_| "World"))
    .subscribe_err(|_| {}, |_| {});
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
