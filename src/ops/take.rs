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
/// observable::from_iter!(0..10).take(5).subscribe(|v| println!("{}", v));
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

pub struct TakeOp<S> {
  source: S,
  count: u32,
}

impl<S> IntoShared for TakeOp<S>
where
  S: IntoShared,
{
  type Shared = TakeOp<S::Shared>;
  fn to_shared(self) -> Self::Shared {
    TakeOp {
      source: self.source.to_shared(),
      count: self.count,
    }
  }
}

impl<Item, Err, Sub, S> RawSubscribable<Item, Err, Sub> for TakeOp<S>
where
  S: RawSubscribable<Item, Err, TakeSubscribe<Sub, LocalSubscription>>,
{
  type Unsub = LocalSubscription;
  fn raw_subscribe(self, subscribe: Sub) -> Self::Unsub {
    let mut subscription = LocalSubscription::default();
    let sub = self.source.raw_subscribe(TakeSubscribe {
      subscribe,
      subscription: subscription.clone(),
      count: self.count,
      hits: 0,
    });
    subscription.add(sub);
    subscription
  }
}

pub struct TakeSubscribe<S, ST> {
  subscribe: S,
  subscription: ST,
  count: u32,
  hits: u32,
}

impl<S, ST> IntoShared for TakeSubscribe<S, ST>
where
  S: IntoShared,
  ST: IntoShared,
{
  type Shared = TakeSubscribe<S::Shared, ST::Shared>;
  fn to_shared(self) -> Self::Shared {
    TakeSubscribe {
      subscribe: self.subscribe.to_shared(),
      subscription: self.subscription.to_shared(),
      count: self.count,
      hits: self.hits,
    }
  }
}

impl<S, ST, Item, Err> Subscribe<Item, Err> for TakeSubscribe<S, ST>
where
  S: Subscribe<Item, Err>,
  ST: SubscriptionLike,
{
  fn on_next(&mut self, value: &Item) {
    if self.hits < self.count {
      self.hits += 1;
      self.subscribe.on_next(value);
      if self.hits == self.count {
        self.subscription.unsubscribe();
      }
    }
  }
  #[inline(always)]
  fn on_error(&mut self, err: &Err) { self.subscribe.on_error(err); }
  #[inline(always)]
  fn on_complete(&mut self) { self.subscribe.on_complete(); }
}

impl<S> Fork for TakeOp<S>
where
  S: Fork,
{
  type Output = TakeOp<S::Output>;
  fn fork(&self) -> Self::Output {
    TakeOp {
      source: self.source.fork(),
      count: self.count,
    }
  }
}

#[cfg(test)]
mod test {
  use super::Take;
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter!(0..100)
      .take(5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert_eq!(completed, true);
  }

  #[test]
  fn take_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    let take5 = observable::from_iter!(0..100).take(5);
    let f1 = take5.fork();
    let f2 = take5.fork();
    f1.take(5).fork().subscribe(|_| nc1 += 1);
    f2.take(5).fork().subscribe(|_| nc2 += 1);

    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn into_shared() {
    observable::from_iter!(0..100)
      .take(5)
      .take(5)
      .to_shared()
      .subscribe(|_| {});
  }
}
