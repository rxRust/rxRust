use crate::observer::error_proxy_impl;
use crate::prelude::*;
use std::collections::VecDeque;

/// Emits only the last `count` values emitted by the source Observable.
///
/// `take_last` returns an Observable that emits only the last `count` values
/// emitted by the source Observable. If the source emits fewer than `count`
/// values then all of its values are emitted.
/// It will not emit values until source Observable complete.
///
/// # Example
/// Take the last 5 seconds of an infinite 1-second interval Observable
///
/// ```
/// # use rxrust::{
///   ops::{TakeLast}, prelude::*,
/// };
///
/// observable::from_iter(0..10).take_last(5).subscribe(|v| println!("{}", v));
///

/// // print logs:
/// // 5
/// // 6
/// // 7
/// // 8
/// // 9
/// ```
///
pub trait TakeLast {
  fn take_last(self, count: usize) -> TakeLastOp<Self>
  where
    Self: Sized,
  {
    TakeLastOp {
      source: self,
      count,
    }
  }
}

impl<O> TakeLast for O {}

#[derive(Clone)]
pub struct TakeLastOp<S> {
  source: S,
  count: usize,
}

macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: TakeLastObserver {
        observer: subscriber.observer,
        count: self.count,
        queue: VecDeque::new(),
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

impl<'a, S> Observable<'a> for TakeLastOp<S>
where
  S: Observable<'a> + 'a,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S> SharedObservable for TakeLastOp<S>
where
  S: SharedObservable,
  S::Item: Send + Sync + 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct TakeLastObserver<O, Item> {
  observer: O,
  count: usize,
  queue: VecDeque<Item>, // TODO: replace VecDeque with RingBuf
}

impl<Item, Err, O> Observer<Item, Err> for TakeLastObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.queue.push_back(value);
    while self.queue.len() > self.count {
      self.queue.pop_front();
    }
  }
  error_proxy_impl!(Err, observer);
  fn complete(&mut self) {
    for value in self.queue.drain(..) {
      self.observer.next(value);
    }
    self.observer.complete();
  }
}

#[cfg(test)]
mod test {
  use super::TakeLast;
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut ticks = vec![];

    observable::from_iter(0..100)
      .take_last(5)
      .subscribe_complete(|v| ticks.push(v), || completed = true);

    assert_eq!(ticks, vec![95, 96, 97, 98, 99]);
    assert_eq!(completed, true);
  }

  #[test]
  fn take_last_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take_last5 = observable::from_iter(0..100).take_last(5);
      let f1 = take_last5.clone();
      let f2 = take_last5.clone();

      f1.take_last(5).clone().subscribe(|_| nc1 += 1);
      f2.take_last(5).clone().subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .take_last(5)
      .take_last(5)
      .to_shared()
      .subscribe(|_| {});
  }
}
