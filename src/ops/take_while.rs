use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;
use observable::observable_proxy_impl;

#[derive(Clone)]
pub struct TakeWhileOp<S, Item> {
  pub(crate) source: S,
  pub(crate) callback: fn(&Item) -> bool,
}

#[doc(hidden)]
macro observable_impl($ subscription: ty, $ ($ marker: ident +) * $ lf: lifetime) {
  fn actual_subscribe < O: Observer < Self::Item, Self::Err > + $ ( $ marker +) * $ lf > (
    self,
    subscriber: Subscriber < O, $ subscription >,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: TakeWhileObserver {
        observer: subscriber.observer,
        subscription: subscriber.subscription.clone(),
        callback: self.callback,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

observable_proxy_impl!(TakeWhileOp, S, Item);

impl<'a, S> LocalObservable<'a> for TakeWhileOp<S, S::Item>
where
  S: LocalObservable<'a>,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<'a, S> SharedObservable for TakeWhileOp<S, S::Item>
  where
      S: SharedObservable,
      S::Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct TakeWhileObserver<O, S, Item> {
  observer: O,
  subscription: S,
  callback: fn(&Item) -> bool,
}

impl<O, U, Item, Err> Observer<Item, Err> for TakeWhileObserver<O, U, Item>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
{
  fn next(&mut self, value: Item) {
    if (self.callback)(&value) {
      self.observer.next(value);
    } else {
      self.observer.complete();
      self.subscription.unsubscribe();
    }
  }
  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take_while::<i32>(|v| v < &5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert_eq!(completed, true);
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
      .to_shared()
      .subscribe(|_| {});
  }
}
