use crate::{
  observer::error_proxy_impl,
  ops::{take::TakeOp, Take},
  prelude::*,
};

/// emit only the first item emitted by an Observable
pub trait First {
  fn first(self) -> TakeOp<Self>
  where
    Self: Sized + Take,
  {
    self.take(1)
  }
}

impl<O> First for O {}

/// emit only the first item (or a default item) emitted by an Observable
pub trait FirstOr<Item> {
  fn first_or(self, default: Item) -> FirstOrOp<TakeOp<Self>, Item>
  where
    Self: Sized,
  {
    FirstOrOp {
      source: self.first(),
      default,
    }
  }
}

impl<Item, O> FirstOr<Item> for O {}

#[derive(Clone)]
pub struct FirstOrOp<S, V> {
  source: S,
  default: V,
}

macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: FirstOrObserver {
        observer: subscriber.observer,
        default: Some(self.default),
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

impl<'a, S, Item> Observable<'a> for FirstOrOp<S, Item>
where
  S: Observable<'a, Item = Item>,
  Item: 'a,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S, Item> SharedObservable for FirstOrOp<S, Item>
where
  S: SharedObservable<Item = Item>,
  Item: Send + Sync + 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct FirstOrObserver<S, T> {
  default: Option<T>,
  observer: S,
}

impl<O, Item, Err> Observer<Item, Err> for FirstOrObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    self.default = None;
  }

  error_proxy_impl!(Err, observer);

  fn complete(&mut self) {
    if let Some(v) = Option::take(&mut self.default) {
      self.observer.next(v)
    }
    self.observer.complete();
  }
}

#[cfg(test)]
mod test {
  use super::{First, FirstOr};
  use crate::prelude::*;

  #[test]
  fn first() {
    let mut completed = 0;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first()
      .subscribe_complete(|_| next_count += 1, || completed += 1);

    assert_eq!(completed, 1);
    assert_eq!(next_count, 1);
  }

  #[test]
  fn first_or() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first_or(100)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 1);
    assert_eq!(completed, true);

    completed = false;
    let mut v = 0;
    observable::empty()
      .first_or(100)
      .subscribe_complete(|value| v = value, || completed = true);

    assert_eq!(completed, true);
    assert_eq!(v, 100);
  }

  #[test]
  fn first_support_fork() {
    let mut value = 0;
    let mut value2 = 0;
    {
      let o = observable::from_iter(1..100).first();
      let o1 = o.clone().first();
      let o2 = o.first();
      o1.subscribe(|v| value = v);
      o2.subscribe(|v| value2 = v);
    }
    assert_eq!(value, 1);
    assert_eq!(value2, 1);
  }
  #[test]
  fn first_or_support_fork() {
    let mut default = 0;
    let mut default2 = 0;
    let o = observable::create(|mut subscriber| {
      subscriber.complete();
    })
    .first_or(100);
    let o1 = o.clone().first_or(0);
    let o2 = o.clone().first_or(0);
    o1.subscribe(|v| default = v);
    o2.subscribe(|v| default2 = v);
    assert_eq!(default, 100);
    assert_eq!(default, 100);
  }
}
