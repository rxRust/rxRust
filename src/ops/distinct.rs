use crate::{impl_local_shared_both, prelude::*};
use std::{cmp::Eq, collections::HashSet, hash::Hash};

#[derive(Clone)]
pub struct DistinctOp<S> {
  pub(crate) source: S,
}

impl<S: Observable> Observable for DistinctOp<S> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> DistinctOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.source.actual_subscribe(DistinctObserver {
      observer: $observer,
      seen: HashSet::new(),
    })
  }
  where
    S: @ctx::Observable,
    S::Item: Eq + Hash + Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)
}
struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

impl<O, Item, Err> Observer for DistinctObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
  Item: Hash + Eq + Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if !self.seen.contains(&value) {
      self.seen.insert(value.clone());
      self.observer.next(value);
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(0..20)
      .map(|v| v % 5)
      .distinct()
      .subscribe(move |v| x.borrow_mut().push(v))
      .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[0, 1, 2, 3, 4]);
  }
  #[test]
  fn shared() {
    observable::from_iter(0..10)
      .distinct()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }
  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_distinct);

  fn bench_distinct(b: &mut bencher::Bencher) { b.iter(smoke); }
}
