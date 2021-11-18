use crate::prelude::*;
use crate::{complete_proxy_impl, error_proxy_impl, is_stopped_proxy_impl};

/// Skips source values until a predicate returns true for the value.
#[derive(Clone)]
pub struct SkipUntilOp<S, F> {
  pub(crate) source: S,
  pub(crate) predicate: F,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $source:ident, $($marker:ident +)* $lf: lifetime) => {
  type Unsub = $source::Unsub;
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let subscriber = Subscriber {
      observer: SkipUntilObserver {
        observer: subscriber.observer,
        predicate: self.predicate,
        done_skipping: false
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}
}

impl<S, F> Observable for SkipUntilOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl<'a, S, F> LocalObservable<'a> for SkipUntilOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut(&S::Item) -> bool + 'a,
{
  observable_impl!(LocalSubscription, S, 'a);
}

impl<S, F> SharedObservable for SkipUntilOp<S, F>
where
  S: SharedObservable,
  F: FnMut(&S::Item) -> bool + Send + Sync + 'static,
{
  observable_impl!(SharedSubscription, S, Send + Sync + 'static);
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

  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
  is_stopped_proxy_impl!(observer);
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
