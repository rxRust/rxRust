use crate::prelude::*;
use std::marker::PhantomData;
use std::sync::Arc;

mod from;
pub use from::*;

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rx_rs
///
pub struct Observable<F, Item, Err> {
  subscribe: F,
  _p: PhantomData<(Item, Err)>,
}

impl<'a, F, Item, Err> Observable<RxFnWrapper<F>, Item, Err>
where
  F: Fn(&mut dyn Observer<Item = Item, Err = Err>),
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

impl<F, Item: 'static, Err: 'static> Multicast for Observable<F, Item, Err>
where
  F: RxFn(&mut dyn Observer<Item = Item, Err = Err>),
{
  type Output = Observable<Arc<F>, Item, Err>;
  fn multicast(self) -> Self::Output {
    Observable {
      subscribe: Arc::new(self.subscribe),
      _p: PhantomData,
    }
  }
}

impl<F, Item: 'static, Err: 'static> Fork for Observable<Arc<F>, Item, Err>
where
  F: RxFn(&mut dyn Observer<Item = Item, Err = Err>),
{
  type Output = Self;
  fn fork(&self) -> Self::Output {
    Observable {
      subscribe: self.subscribe.clone(),
      _p: PhantomData,
    }
  }
}

impl<F, Item: 'static, Err: 'static> ImplSubscribable
  for Observable<F, Item, Err>
where
  F: RxFn(&mut dyn Observer<Item = Item, Err = Err>),
{
  type Item = Item;
  type Err = Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'static,
    error: Option<impl Fn(&Self::Err) + 'static>,
    complete: Option<impl Fn() + 'static>,
  ) -> Box<dyn Subscription> {
    let mut subscriber = Subscriber::new(next);
    if error.is_some() {
      subscriber.on_error(error.unwrap())
    };
    if complete.is_some() {
      subscriber.on_complete(complete.unwrap())
    };
    self.subscribe.call((&mut subscriber,));
    Box::new(subscriber)
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

    Observable::new(|subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error(&"never dispatch error");
    })
    .subscribe_err_complete(
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
    let o = Observable::new(|subscriber| {
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
    let observable = Observable::new(|s| {
      s.next(&0);
      s.error(&"");
      s.complete();
    });
    let _o = observable.multicast().fork().fork().fork();
  }
}
