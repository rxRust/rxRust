use crate::observer::observer_error_proxy_impl;
use crate::ops::SharedOp;
use crate::prelude::*;
use std::collections::VecDeque;
use std::marker::PhantomData;

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
pub trait TakeLast<Item> {
  fn take_last(self, count: usize) -> TakeLastOp<Self, Item>
  where
    Self: Sized,
  {
    TakeLastOp {
      source: self,
      count,
      _p: PhantomData,
    }
  }
}

impl<O, Item> TakeLast<Item> for O {}

pub struct TakeLastOp<S, Item> {
  source: S,
  count: usize,
  _p: PhantomData<Item>,
}

impl<S, Item> IntoShared for TakeLastOp<S, Item>
where
  S: IntoShared,
  Item: Send + Sync + 'static,
{
  type Shared = SharedOp<TakeLastOp<S::Shared, Item>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(TakeLastOp {
      source: self.source.to_shared(),
      count: self.count,
      _p: PhantomData,
    })
  }
}

impl<O, U, S, Item> RawSubscribable<Subscriber<O, U>> for TakeLastOp<S, Item>
where
  S: RawSubscribable<Subscriber<TakeLastObserver<O, U, Item>, U>>,
  U: SubscriptionLike + Clone + 'static,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: TakeLastObserver {
        observer: subscriber.observer,
        subscription: subscriber.subscription.clone(),
        count: self.count,
        queue: VecDeque::new(),
      },
      subscription: subscriber.subscription,
    };
    self.source.raw_subscribe(subscriber)
  }
}

pub struct TakeLastObserver<O, S, Item> {
  observer: O,
  subscription: S,
  count: usize,
  queue: VecDeque<Item>, // TODO: replace VecDeque with RingBuf
}

impl<S, ST, Item> IntoShared for TakeLastObserver<S, ST, Item>
where
  S: IntoShared,
  ST: IntoShared,
  Item: Send + Sync + 'static,
{
  type Shared = TakeLastObserver<S::Shared, ST::Shared, Item>;
  fn to_shared(self) -> Self::Shared {
    TakeLastObserver {
      observer: self.observer.to_shared(),
      subscription: self.subscription.to_shared(),
      count: self.count,
      queue: VecDeque::new(),
    }
  }
}

impl<Item, O, U> ObserverNext<Item> for TakeLastObserver<O, U, Item>
where
  O: ObserverNext<Item>,
{
  fn next(&mut self, value: Item) {
    self.queue.push_back(value);
    while self.queue.len() > self.count {
      self.queue.pop_front();
    }
  }
}

impl<Item, O, U> ObserverComplete for TakeLastObserver<O, U, Item>
where
  O: ObserverNext<Item> + ObserverComplete,
{
  fn complete(&mut self) {
    for value in self.queue.drain(..) {
      self.observer.next(value);
    }
    self.observer.complete();
  }
}

observer_error_proxy_impl!(
  TakeLastObserver<O, U, Item>, O, observer, <O, U, Item>
);

impl<S, Item> Fork for TakeLastOp<S, Item>
where
  S: Fork,
{
  type Output = TakeLastOp<S::Output, Item>;
  fn fork(&self) -> Self::Output {
    TakeLastOp {
      source: self.source.fork(),
      count: self.count,
      _p: PhantomData,
    }
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
      let f1 = take_last5.fork();
      let f2 = take_last5.fork();

      f1.take_last(5).fork().subscribe(|_| nc1 += 1);
      f2.take_last(5).fork().subscribe(|_| nc2 += 1);
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
