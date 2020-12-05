use crate::{observer::error_proxy_impl, prelude::*};

#[derive(Clone)]
pub struct FirstOrOp<S, V> {
  pub(crate) source: S,
  pub(crate) default: V,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
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

impl<S, Item> Observable for FirstOrOp<S, Item>
where
  S: Observable<Item = Item>,
{
  type Item = Item;
  type Err = S::Err;
}

impl<'a, S, Item> LocalObservable<'a> for FirstOrOp<S, Item>
where
  S: LocalObservable<'a, Item = Item>,
  Item: 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S, Item> SharedObservable for FirstOrOp<S, Item>
where
  S: SharedObservable<Item = Item>,
  Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct FirstOrObserver<S, T> {
  default: Option<T>,
  observer: S,
}

impl<O, Item, Err> Observer for FirstOrObserver<O, Item>
where
  O: Observer<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
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

  #[inline]
  fn is_stopped(&self) -> bool { self.observer.is_stopped() }
}

#[cfg(test)]
mod test {
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
