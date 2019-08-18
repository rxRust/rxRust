use crate::prelude::*;
use std::marker::PhantomData;
use std::sync::Arc;

mod from;
pub use from::*;
mod from_future;
pub use from_future::{from_future, from_future_with_err};
mod once;
pub use once::{once, ObservableOnce};

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rxrust
///
pub struct Observable<F, Item, Err> {
  subscribe: F,
  _p: PhantomData<(Item, Err)>,
}

impl<F, Item, Err> Observable<RxFnWrapper<F>, Item, Err>
where
  F: Fn(Box<dyn Observer<Item, Err> + Send>),
{
  /// param `subscribe`: the function that is called when the Observable is
  /// initially subscribed to. This function is given a Subscriber, to which
  /// new values can be `next`ed, or an `error` method can be called to raise
  /// an error, or `complete` can be called to notify of a successful
  /// completion.
  pub fn new(subscribe: F) -> Self {
    Self {
      subscribe: RxFnWrapper::new(subscribe),
      _p: PhantomData,
    }
  }
}

impl<F, Item, Err> Multicast for Observable<F, Item, Err>
where
  F: RxFn(Box<dyn Observer<Item, Err> + Send>) + Send + Sync,
{
  type Output = Observable<Arc<F>, Item, Err>;
  fn multicast(self) -> Self::Output {
    Observable {
      subscribe: Arc::new(self.subscribe),
      _p: PhantomData,
    }
  }
}

impl<F, Item, Err> Fork for Observable<Arc<F>, Item, Err>
where
  F: RxFn(Box<dyn Observer<Item, Err> + Send>) + Send + Sync,
{
  type Output = Self;
  fn fork(&self) -> Self::Output {
    Observable {
      subscribe: self.subscribe.clone(),
      _p: PhantomData,
    }
  }
}

impl<F, Item, Err> RawSubscribable for Observable<F, Item, Err>
where
  F: RxFn(Box<dyn Observer<Item, Err> + Send>) + Send + Sync,
{
  type Item = Item;
  type Err = Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let subscriber = Subscriber::new(subscribe);

    let subscription = subscriber.clone_subscription();
    self.subscribe.call((Box::new(subscriber),));
    Box::new(subscription)
  }
}

#[cfg(test)]
mod test {
  use crate::ops::{Fork, Multicast};
  use crate::prelude::*;
  use std::sync::{Arc, Mutex};

  #[test]
  fn proxy_call() {
    let next = Arc::new(Mutex::new(0));
    let err = Arc::new(Mutex::new(0));
    let complete = Arc::new(Mutex::new(0));
    let c_next = next.clone();
    let c_err = err.clone();
    let c_complete = complete.clone();

    Observable::new(|mut subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error(&"never dispatch error");
    })
    .subscribe_all(
      move |_| *next.lock().unwrap() += 1,
      move |_: &&str| *err.lock().unwrap() += 1,
      move || *complete.lock().unwrap() += 1,
    );

    assert_eq!(*c_next.lock().unwrap(), 3);
    assert_eq!(*c_complete.lock().unwrap(), 1);
    assert_eq!(*c_err.lock().unwrap(), 0);
  }

  #[test]
  fn support_fork() {
    let o = Observable::new(|mut subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.next(&4);
      subscriber.error(&"");
    })
    .multicast();
    let sum1 = Arc::new(Mutex::new(0));
    let sum2 = Arc::new(Mutex::new(0));
    let c_sum1 = sum1.clone();
    let c_sum2 = sum2.clone();
    o.fork().subscribe(move |v| *sum1.lock().unwrap() += v);
    o.fork().subscribe(move |v| *sum2.lock().unwrap() += v);

    assert_eq!(*c_sum1.lock().unwrap(), 10);
    assert_eq!(*c_sum2.lock().unwrap(), 10);
  }

  #[test]
  fn observable_fork() {
    let observable = Observable::new(|mut s| {
      s.next(&0);
      s.error(&"");
      s.complete();
    });
    let _o = observable.multicast().fork().fork().fork();
  }
}
