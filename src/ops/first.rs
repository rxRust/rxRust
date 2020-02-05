use crate::{
  observer::observer_error_proxy_impl,
  ops::{take::TakeOp, SharedOp, Take},
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

pub struct FirstOrOp<S, V> {
  source: S,
  default: V,
}

impl<O, U, S, T> RawSubscribable<Subscriber<O, U>> for FirstOrOp<S, T>
where
  S: RawSubscribable<Subscriber<FirstOrObserver<O, T>, U>>,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: FirstOrObserver {
        observer: subscriber.observer,
        default: Some(self.default),
      },
      subscription: subscriber.subscription,
    };
    self.source.raw_subscribe(subscriber)
  }
}

impl<S, V> IntoShared for FirstOrOp<S, V>
where
  S: IntoShared,
  V: Send + Sync + 'static,
{
  type Shared = SharedOp<FirstOrOp<S::Shared, V>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(FirstOrOp {
      source: self.source.to_shared(),
      default: self.default,
    })
  }
}

pub struct FirstOrObserver<S, T> {
  default: Option<T>,
  observer: S,
}

impl<O, Item> ObserverNext<Item> for FirstOrObserver<O, Item>
where
  O: ObserverNext<Item>,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    self.default = None;
  }
}

observer_error_proxy_impl!(FirstOrObserver<O, Item>, O, observer, <O, Item>);

impl<O, Item> ObserverComplete for FirstOrObserver<O, Item>
where
  O: ObserverComplete + ObserverNext<Item>,
{
  fn complete(&mut self) {
    if let Some(v) = Option::take(&mut self.default) {
      self.observer.next(v)
    }
    self.observer.complete();
  }
}

impl<S, V> IntoShared for FirstOrObserver<S, V>
where
  S: IntoShared,
  V: Send + Sync + 'static,
{
  type Shared = FirstOrObserver<S::Shared, V>;
  fn to_shared(self) -> Self::Shared {
    FirstOrObserver {
      observer: self.observer.to_shared(),
      default: self.default,
    }
  }
}

impl<S, T> Fork for FirstOrOp<S, T>
where
  S: Fork,
  T: Clone,
{
  type Output = FirstOrOp<S::Output, T>;
  fn fork(&self) -> Self::Output {
    FirstOrOp {
      source: self.source.fork(),
      default: self.default.clone(),
    }
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
      let o1 = o.fork().first();
      let o2 = o.fork().first();
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
    let o1 = o.fork().first_or(0);
    let o2 = o.fork().first_or(0);
    o1.subscribe(|v| default = v);
    o2.subscribe(|v| default2 = v);
    assert_eq!(default, 100);
    assert_eq!(default, 100);
  }

  #[test]
  fn fork_and_shared() {
    observable::of(0)
      .first_or(0)
      .fork()
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});

    observable::of(0)
      .first()
      .fork()
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
