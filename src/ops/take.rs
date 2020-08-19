use crate::observer::{
  complete_proxy_impl, error_proxy_impl, is_stopped_proxy_impl,
};
use crate::prelude::*;
use observable::observable_proxy_impl;

#[derive(Clone)]
pub struct TakeOp<S> {
  pub(crate) source: S,
  pub(crate) count: u32,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: TakeObserver {
        observer: subscriber.observer,
        subscription: subscriber.subscription.clone(),
        count: self.count,
        hits: 0,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

observable_proxy_impl!(TakeOp, S);

impl<'a, S> LocalObservable<'a> for TakeOp<S>
where
  S: LocalObservable<'a>,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S> SharedObservable for TakeOp<S>
where
  S: SharedObservable,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct TakeObserver<O, S> {
  observer: O,
  subscription: S,
  count: u32,
  hits: u32,
}

impl<O, U, Item, Err> Observer<Item, Err> for TakeObserver<O, U>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
{
  fn next(&mut self, value: Item) {
    if self.hits < self.count {
      self.hits += 1;
      self.observer.next(value);
      if self.hits == self.count {
        self.complete();
        self.subscription.unsubscribe();
      }
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
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take(5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert_eq!(completed, true);
  }

  #[test]
  fn take_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take5 = observable::from_iter(0..100).take(5);
      let f1 = take5.clone();
      let f2 = take5;

      f1.take(5).subscribe(|_| nc1 += 1);
      f2.take(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .take(5)
      .take(5)
      .to_shared()
      .subscribe(|_| {});
  }
}
