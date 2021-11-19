use crate::{impl_local_shared_both, prelude::*};

/// Skips source values until a predicate returns true for the value.
#[derive(Clone)]
pub struct SkipUntilOp<S, F> {
  pub(crate) source: S,
  pub(crate) predicate: F,
}

impl<S, F> Observable for SkipUntilOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, F> SkipUntilOp<S, F>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let observer = SkipUntilObserver {
      observer: $observer,
      predicate: $self.predicate,
      done_skipping: false
    };
    $self.source.actual_subscribe(observer)
  }
  where
    S: @ctx::Observable,
    F: FnMut(&S::Item) -> bool +
      @ctx::local_only('o)
      @ctx::shared_only(Send + Sync + 'static)
}

pub struct SkipUntilObserver<O, F> {
  observer: O,
  predicate: F,
  done_skipping: bool,
}

impl<O, Item, Err, F> Observer for SkipUntilObserver<O, F>
where
  O: Observer<Item = Item, Err = Err>,
  F: FnMut(&Item) -> bool,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Item) {
    if self.done_skipping {
      self.observer.next(value);
    } else if (self.predicate)(&value) {
      self.observer.next(value);
      self.done_skipping = true;
    }
  }

  #[inline]
  fn error(&mut self, err: Err) { self.observer.error(err); }

  #[inline]
  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut items = vec![];

    observable::from_iter(0..100)
      .skip_until(|v| v >= &50)
      .subscribe_complete(|v| items.push(v), || completed = true);

    assert_eq!((50..100).collect::<Vec<i32>>(), items);
    assert!(completed);
  }

  #[test]
  fn skip_until_support_fork() {
    let mut items1 = vec![];
    let mut items2 = vec![];

    {
      let skip_until = observable::from_iter(0..10).skip_until(|v| v >= &5);
      let f1 = skip_until.clone();
      let f2 = skip_until;

      f1.skip_until(|v| v >= &5).subscribe(|v| items1.push(v));
      f2.skip_until(|v| v >= &5).subscribe(|v| items2.push(v));
    }
    assert_eq!(items1, items2);
    assert_eq!((5..10).collect::<Vec<i32>>(), items1);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_skip_until);

  fn bench_skip_until(b: &mut bencher::Bencher) { b.iter(base_function); }
}
