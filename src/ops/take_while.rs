use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
#[derive(Clone)]
pub struct TakeWhileOp<S, F> {
  pub(crate) source: S,
  pub(crate) callback: F,
  pub(crate) inclusive: bool,
}

impl<S, F> Observable for TakeWhileOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, F> TakeWhileOp<S, F>;
  type Unsub = @ctx::Rc<ProxySubscription<S::Unsub>>;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let subscription = $ctx::Rc::own(ProxySubscription::default());
    let observer = TakeWhileObserver {
      observer: $observer,
      subscription: subscription.clone(),
      callback: $self.callback,
      inclusive: $self.inclusive,
    };
    let s = $self.source.actual_subscribe(observer);
    subscription.rc_deref_mut().proxy(s);
    subscription
  }
  where
    S: @ctx::Observable,
    @ctx::local_only(
      F: FnMut(&S::Item) -> bool + 'o,
      S::Unsub: 'o
    )
    @ctx::shared_only(F: FnMut(&S::Item) -> bool + Send + Sync + 'static )
}

pub struct TakeWhileObserver<O, S, F> {
  observer: O,
  subscription: S,
  callback: F,
  inclusive: bool,
}

impl<O, U, Item, Err, F> Observer for TakeWhileObserver<O, U, F>
where
  O: Observer<Item = Item, Err = Err>,
  U: SubscriptionLike,
  F: FnMut(&Item) -> bool,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if (self.callback)(&value) {
      self.observer.next(value);
    } else {
      if self.inclusive {
        self.observer.next(value);
      }
      self.observer.complete();
      self.subscription.unsubscribe();
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take_while(|v| v < &5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn inclusive_case() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take_while_inclusive(|v| v < &5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 6);
    assert!(completed);
  }

  #[test]
  fn take_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take_while5 = observable::from_iter(0..100).take_while(|v| v < &5);
      let f1 = take_while5.clone();
      let f2 = take_while5;

      f1.take_while(|v| v < &5).subscribe(|_| nc1 += 1);
      f2.take_while(|v| v < &5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .take_while(|v| v < &5)
      .take_while(|v| v < &5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take_while);

  fn bench_take_while(b: &mut bencher::Bencher) { b.iter(base_function); }
}
