use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;
/// Emits only the first `count` values emitted by the source Observable.
///
/// `take` returns an Observable that emits only the first `count` values
/// emitted by the source Observable. If the source emits fewer than `count`
/// values then all of its values are emitted. After that, it completes,
/// regardless if the source completes.
///
/// # Example
/// Take the first 5 seconds of an infinite 1-second interval Observable
///
/// ```
/// # use rxrust::{
///   ops::{Take}, prelude::*,
/// };
///
/// observable::from_iter(0..10).take(5).subscribe(|v| println!("{}", v));
///

/// // print logs:
/// // 0
/// // 1
/// // 2
/// // 3
/// // 4
/// ```
///
pub trait Take {
  fn take(self, count: u32) -> TakeOp<Self>
  where
    Self: Sized,
  {
    TakeOp {
      source: self,
      count,
    }
  }
}

impl<O> Take for O {}

#[derive(Clone)]
pub struct TakeOp<S> {
  source: S,
  count: u32,
}

impl<'a, S> Observable<'a> for TakeOp<S>
where
  S: Observable<'a>,
{
  type Item = S::Item;
  type Err = S::Err;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + 'a,
    U: SubscriptionLike + Clone + 'static,
  >(
    self,
    subscriber: Subscriber<O, U>,
  ) -> U {
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

auto_impl_shared_observable!(TakeOp<S>, <S>);

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
}

#[cfg(test)]
mod test {
  use super::Take;
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
      let f2 = take5.clone();

      f1.take(5).clone().subscribe(|_| nc1 += 1);
      f2.take(5).clone().subscribe(|_| nc2 += 1);
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
