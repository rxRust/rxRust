use crate::{impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct SkipWhileOp<S, F> {
  pub(crate) source: S,
  pub(crate) predicate: F,
}

impl<S, F> Observable for SkipWhileOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, F> SkipWhileOp<S, F>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let observer = SkipWhileObserver {
      observer: $observer,
      predicate: $self.predicate,
      done_skip: false
    };
    $self.source.actual_subscribe(observer)
  }
  where
    S: @ctx::Observable,
    F: FnMut(&S::Item) -> bool +
      @ctx::local_only('o)
      @ctx::shared_only(Send + Sync + 'static)
}

pub struct SkipWhileObserver<O, F> {
  observer: O,
  predicate: F,
  done_skip: bool,
}

impl<O, Item, Err, F> Observer for SkipWhileObserver<O, F>
where
  O: Observer<Item = Item, Err = Err>,
  F: FnMut(&Item) -> bool,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Item) {
    if self.done_skip {
      self.observer.next(value);
    } else if !(self.predicate)(&value) {
      self.done_skip = true;
      self.observer.next(value);
    }
  }

  #[inline]
  fn error(&mut self, err: Err) {
    self.observer.error(err);
  }

  #[inline]
  fn complete(&mut self) {
    self.observer.complete()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .skip_while(|v| v != &95)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn skip_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip_while5 = observable::from_iter(0..100).skip_while(|v| v != &95);
      let f1 = skip_while5.clone();
      let f2 = skip_while5;

      f1.skip_while(|v| v != &95).subscribe(|_| nc1 += 1);
      f2.skip_while(|v| v != &95).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn ininto_shared() {
    observable::from_iter(0..100)
      .skip_while(|v| v != &95)
      .skip_while(|v| v != &95)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_skip_while);

  fn bench_skip_while(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
