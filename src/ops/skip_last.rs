use crate::observer::observer_error_proxy_impl;
use crate::ops::SharedOp;
use crate::prelude::*;
use std::collections::VecDeque;
use std::marker::PhantomData;

/// Ignore the last `count` values emitted by the source Observable.
///
/// `skip_last` returns an Observable that ignore the last `count` values
/// emitted by the source Observable. If the source emits fewer than `count`
/// values then 0 of its values are emitted.
/// It will not emit values until source Observable complete.
///
/// # Example
/// Skip the last 5 seconds of an infinite 1-second interval Observable
///
/// ```
/// # use rxrust::{
///   ops::{SkipLast}, prelude::*,
/// };
///
/// observable::from_iter(0..10).skip_last(5).subscribe(|v| println!("{}", v));
///

/// // print logs:
/// // 0
/// // 1
/// // 2
/// // 3
/// // 4
/// ```
///
pub trait SkipLast<Item> {
  fn skip_last(self, count: usize) -> SkipLastOp<Self, Item>
  where
    Self: Sized,
  {
    SkipLastOp {
      source: self,
      count,
      _p: PhantomData,
    }
  }
}

impl<O, Item> SkipLast<Item> for O {}

pub struct SkipLastOp<S, Item> {
  source: S,
  count: usize,
  _p: PhantomData<Item>,
}

impl<S, Item> IntoShared for SkipLastOp<S, Item>
where
  S: IntoShared,
  Item: Send + Sync + 'static,
{
  type Shared = SharedOp<SkipLastOp<S::Shared, Item>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(SkipLastOp {
      source: self.source.to_shared(),
      count: self.count,
      _p: PhantomData,
    })
  }
}

impl<O, U, S, Item> RawSubscribable<Subscriber<O, U>> for SkipLastOp<S, Item>
where
  S: RawSubscribable<Subscriber<SkipLastObserver<O, U, Item>, U>>,
  U: SubscriptionLike + Clone + 'static,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: SkipLastObserver {
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

pub struct SkipLastObserver<O, S, Item> {
  observer: O,
  subscription: S,
  count: usize,
  queue: VecDeque<Item>,
}

impl<S, ST, Item> IntoShared for SkipLastObserver<S, ST, Item>
where
  S: IntoShared,
  ST: IntoShared,
  Item: Send + Sync + 'static,
{
  type Shared = SkipLastObserver<S::Shared, ST::Shared, Item>;
  fn to_shared(self) -> Self::Shared {
    SkipLastObserver {
      observer: self.observer.to_shared(),
      subscription: self.subscription.to_shared(),
      count: self.count,
      queue: VecDeque::new(),
    }
  }
}

impl<Item, O, U> ObserverNext<Item> for SkipLastObserver<O, U, Item>
where
  O: ObserverNext<Item>,
{
  fn next(&mut self, value: Item) {
    self.queue.push_back(value);
  }
}

impl<Item, O, U> ObserverComplete for SkipLastObserver<O, U, Item>
where
  O: ObserverNext<Item> + ObserverComplete,
{
  fn complete(&mut self) {
    if self.count <= self.queue.len() {
      let skip_index = self.queue.len() - self.count;
      for value in self.queue.drain(..skip_index) {
        self.observer.next(value);
      }
    }
    self.observer.complete();
  }
}

observer_error_proxy_impl!(
  SkipLastObserver<O, U, Item>, O, observer, <O, U, Item>
);

impl<S, Item> Fork for SkipLastOp<S, Item>
where
  S: Fork,
{
  type Output = SkipLastOp<S::Output, Item>;
  fn fork(&self) -> Self::Output {
    SkipLastOp {
      source: self.source.fork(),
      count: self.count,
      _p: PhantomData,
    }
  }
}

#[cfg(test)]
mod test {
  use super::SkipLast;
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut ticks = vec![];

    observable::from_iter(0..10)
      .skip_last(5)
      .subscribe_complete(|v| ticks.push(v), || completed = true);

    assert_eq!(ticks, vec![0, 1, 2, 3, 4]);
    assert_eq!(completed, true);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut ticks = vec![];

    observable::from_iter(0..10)
      .skip_last(11)
      .subscribe_complete(|v| ticks.push(v), || completed = true);

    assert_eq!(ticks, vec![]);
    assert_eq!(completed, true);
  }

  #[test]
  fn skip_last_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let skip_last5 = observable::from_iter(0..100).skip_last(5);
      let f1 = skip_last5.fork();
      let f2 = skip_last5.fork();

      f1.skip_last(5).fork().subscribe(|_| nc1 += 1);
      f2.skip_last(5).fork().subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 90);
    assert_eq!(nc2, 90);
  }

  #[test]
  fn into_shared() {
    observable::from_iter(0..100)
      .skip_last(5)
      .skip_last(5)
      .to_shared()
      .subscribe(|_| {});
  }
}
