use crate::{impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct DefaultIfEmptyOp<S>
where
  S: Observable,
{
  pub(crate) source: S,
  pub(crate) is_empty: bool,
  pub(crate) default_value: S::Item,
}

impl<S: Observable> Observable for DefaultIfEmptyOp<S> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> DefaultIfEmptyOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DefaultIfEmptyObserver {
      observer: $observer,
      is_empty: $self.is_empty,
      default_value: $self.default_value,
    })
  }
  where
    S: @ctx::Observable,
    S::Item: Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)

}

pub struct DefaultIfEmptyObserver<O, Item> {
  observer: O,
  is_empty: bool,
  default_value: Item,
}

impl<Item, Err, O> Observer for DefaultIfEmptyObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    if self.is_empty {
      self.is_empty = false;
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) {
    if self.is_empty {
      self.observer.next(self.default_value.clone());
    }
    self.observer.complete()
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
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 10);
    assert!(completed);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut value = 0;

    observable::empty()
      .default_if_empty(5)
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 5);
    assert!(completed);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .default_if_empty(5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn ininto_shared_empty() {
    observable::empty()
      .default_if_empty(5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench_base() { bench_b(); }

  benchmark_group!(bench_b, bench_base_function);

  fn bench_base_function(b: &mut Bencher) { b.iter(base_function); }

  #[test]
  fn bench_empty() { bench_e(); }

  benchmark_group!(bench_e, bench_empty_function);

  fn bench_empty_function(b: &mut Bencher) { b.iter(base_empty_function); }
}
