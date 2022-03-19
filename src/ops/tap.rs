use crate::{impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct TapOp<S, M> {
  pub(crate) source: S,
  pub(crate) func: M,
}

impl<Item, S, M> Observable for TapOp<S, M>
where
  S: Observable<Item = Item>,
  M: FnMut(&Item),
{
  type Item = Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, M>  TapOp<S, M>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let func = $self.func;
    $self.source.actual_subscribe(TapObserver {
      observer: $observer,
      func,
      _marker: TypeHint::new(),
    })
  }
  where
    S: @ctx::Observable,
    S::Item: @ctx::local_only('o) @ctx::shared_only('static),
    M: FnMut(&S::Item)
      + @ctx::local_only('o) @ctx::shared_only( Send + Sync + 'static)
}

#[derive(Clone)]
pub struct TapObserver<O, F, Item> {
  observer: O,
  func: F,
  _marker: TypeHint<*const Item>,
}

impl<Item, Err, O, F> Observer for TapObserver<O, F, Item>
where
  O: Observer<Item = Item, Err = Err>,
  F: FnMut(&Item),
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    (self.func)(&value);
    self.observer.next(value)
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn primitive_type() {
    let mut i = 0;
    let mut v = 0;
    observable::from_iter(100..101)
      .tap(|i| v = *i)
      .subscribe(|v| i += v);
    assert_eq!(i, 100);
    assert_eq!(v, 100);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).map(|v| v);
    m.tap(|v| println!("v: {}", v))
      .into_shared()
      .subscribe(|_| {});

    // type mapped to other type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).map(|_v| 1);
    m.tap(|v| println!("v: {}", v))
      .into_shared()
      .subscribe(|_| {});

    // ref to ref can fork
    let m = observable::of(&1).map(|v| v);
    m.tap(|v| println!("v: {}", v))
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn benchmark() { do_bench(); }

  benchmark_group!(do_bench, bench);

  fn bench(b: &mut bencher::Bencher) { b.iter(primitive_type); }
}
