use crate::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{atomic::AtomicBool, Arc};

// mod from;
// pub use from::*;
// pub(crate) mod from_future;
// pub use from_future::{from_future, from_future_with_err};·
// pub(crate) mod once;
// pub use once::{once, ObservableOnce};
// pub(crate) mod interval;
// pub use interval::{interval, interval_at};

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rxrust
///
#[derive(Clone)]
pub struct Observable<F>(F);

impl<F> Observable<F> {
  /// param `subscribe`: the function that is called when the Observable is
  /// initially subscribed to. This function is given a Subscriber, to which
  /// new values can be `next`ed, or an `error` method can be called to raise
  /// an error, or `complete` can be called to notify of a successful
  /// completion.
  pub fn new(subscribe: F) -> Self { Self(subscribe) }
}

impl<F, Item, Err, S> RawSubscribable<Item, Err, S> for Observable<F>
where
  S: Subscribe<Item, Err>,
  F: Fn(&mut dyn Observer<Item, Err>),
{
  type Unsub = Rc<Cell<bool>>;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let mut subscriber = Subscriber::from_subscribe(subscribe);

    let subscription = subscriber.clone_subscription();
    (self.0)(&mut subscriber);
    subscription
  }
}

pub struct SharedObservable<F>(F);

impl<F, Item, Err, S> RawSubscribable<Item, Err, S> for SharedObservable<F>
where
  S: Subscribe<Item, Err> + IntoSharedSubscribe<Item, Err>,
  F: Fn(&mut dyn Observer<Item, Err>),
{
  type Unsub = Arc<AtomicBool>;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let mut subscriber = Subscriber::from_subscribe(subscribe).to_shared();

    let subscription = subscriber.clone_subscription();
    (self.0)(&mut subscriber);
    subscription
  }
}

impl<F> IntoSharedSubscribable for Observable<F>
where
  F: Send + Sync + 'static,
{
  type Shared = SharedObservable<F>;
  fn to_shared(self) -> Self::Shared { SharedObservable(self.0) }
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

    Observable::new(|subscriber: &mut dyn Observer<_, _>| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error(&"never dispatch error");
    })
    .to_shared()
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
    let o = Observable::new(|subscriber: &mut dyn Observer<_, _>| {
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
    let observable = Observable::new(|s: &mut dyn Observer<_, _>| {
      s.next(&0);
      s.error(&"");
      s.complete();
    });
    let _o = observable.multicast().fork().fork().fork();
  }
}
