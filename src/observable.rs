use crate::prelude::*;

mod from;
pub use from::*;
pub(crate) mod from_future;
pub use from_future::{from_future, from_future_with_err};
// pub(crate) mod interval;
// pub use interval::{interval, interval_at};

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rxrust
///
#[derive(Clone)]

pub struct Observable<F>(F);

pub trait ObservableDispatch<Item, Err, S> {
  fn dispatch(self, s: S);
}

impl<F> Observable<F> {
  /// param `subscribe`: the function that is called when the Observable is
  /// initially subscribed to. This function is given a Subscriber, to which
  /// new values can be `next`ed, or an `error` method can be called to raise
  /// an error, or `complete` can be called to notify of a successful
  /// completion.
  pub fn new<S, U>(subscribe: F) -> Self
  where
    F: FnOnce(Subscriber<S, U>),
  {
    Self(subscribe)
  }
}

impl<F, S, Item, Err> ObservableDispatch<Item, Err, S> for Observable<F>
where
  F: FnOnce(S),
  S: Subscribe<Item, Err>,
{
  #[inline(always)]
  fn dispatch(self, s: S) { (self.0)(s); }
}

impl<F> Fork for Observable<F>
where
  F: Clone,
{
  type Output = ForkObservable<Observable<F>>;
  #[inline(always)]
  fn fork(&self) -> Self::Output { ForkObservable(self.clone()) }
}

impl<F, Item, Err, S> RawSubscribable<Item, Err, S> for Observable<F>
where
  S: Subscribe<Item, Err>,
  F: FnOnce(Subscriber<S, LocalSubscription>),
{
  type Unsub = LocalSubscription;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let subscriber = Subscriber::from_subscribe(subscribe);
    let subscription = subscriber.clone_subscription();
    self.dispatch(subscriber);
    subscription
  }
}

#[derive(Clone)]
pub struct SharedObservable<F>(F);

impl<F> Fork for SharedObservable<F>
where
  F: Clone,
{
  type Output = ForkObservable<SharedObservable<F>>;
  #[inline(always)]
  fn fork(&self) -> Self::Output { ForkObservable(self.clone()) }
}

impl<O, Item, Err, S> RawSubscribable<Item, Err, S> for SharedObservable<O>
where
  S: Subscribe<Item, Err> + IntoSharedSubscribe<Item, Err>,
  O: ObservableDispatch<Item, Err, Subscriber<S::Shared, SharedSubscription>>,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let subscriber = Subscriber::from_subscribe(subscribe).to_shared();
    let subscription = subscriber.clone_subscription();
    self.0.dispatch(subscriber);
    subscription
  }
}

impl<O, S, Item, Err> ObservableDispatch<Item, Err, S> for SharedObservable<O>
where
  O: ObservableDispatch<Item, Err, S::Shared>,
  S: IntoSharedSubscribe<Item, Err>,
{
  fn dispatch(self, s: S) { self.0.dispatch(s.to_shared()) }
}

impl<F> IntoSharedSubscribable for Observable<F>
where
  F: Send + Sync + 'static,
{
  type Shared = SharedObservable<Observable<F>>;
  fn to_shared(self) -> Self::Shared { SharedObservable(self) }
}

#[derive(Clone)]
pub struct ForkObservable<F>(F);

impl<F> IntoSharedSubscribable for ForkObservable<F>
where
  F: Send + Sync + 'static,
{
  type Shared = SharedObservable<ForkObservable<F>>;
  fn to_shared(self) -> Self::Shared { SharedObservable(self) }
}

impl<F> Fork for ForkObservable<F>
where
  F: Clone,
{
  type Output = ForkObservable<F>;
  #[inline(always)]
  fn fork(&self) -> Self::Output { self.clone() }
}

impl<O, Item, Err, S> ObservableDispatch<Item, Err, S>
  for ForkObservable<ForkObservable<O>>
where
  ForkObservable<O>: ObservableDispatch<Item, Err, S>,
{
  #[inline(always)]
  fn dispatch(self, s: S) { self.0.dispatch(s) }
}

impl<O, S, U, Item, Err> ObservableDispatch<Item, Err, Subscriber<S, U>>
  for ForkObservable<SharedObservable<O>>
where
  SharedObservable<O>: ObservableDispatch<
    Item,
    Err,
    Subscriber<Box<dyn Subscribe<Item, Err> + Send + Sync>, SharedSubscription>,
  >,
  S: Subscribe<Item, Err> + Send + Sync + 'static,
  Subscriber<S, U>: SubscriptionLike,
  U: IntoSharedSubscription,
{
  fn dispatch(self, s: Subscriber<S, U>) {
    let subscribe: Box<dyn Subscribe<Item, Err> + Send + Sync> =
      Box::new(s.subscribe);
    self.0.dispatch(Subscriber {
      subscribe,
      stopped: s.stopped.to_shared(),
    })
  }
}

impl<'a, F, S, U, Item, Err> ObservableDispatch<Item, Err, Subscriber<S, U>>
  for ForkObservable<Observable<F>>
where
  S: Subscribe<Item, Err> + 'a,
  F: FnOnce(Subscriber<Box<dyn Subscribe<Item, Err> + 'a>, U>),
{
  fn dispatch(self, s: Subscriber<S, U>) {
    let subscribe: Box<dyn Subscribe<Item, Err> + 'a> = Box::new(s.subscribe);
    (self.0).dispatch(Subscriber {
      subscribe,
      stopped: s.stopped,
    })
  }
}

impl<'a, F, Item, Err, S> RawSubscribable<Item, Err, S>
  for ForkObservable<Observable<F>>
where
  S: Subscribe<Item, Err> + 'a,
  F: FnOnce(Subscriber<Box<dyn Subscribe<Item, Err> + 'a>, LocalSubscription>),
{
  type Unsub = LocalSubscription;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let subscribe: Box<dyn Subscribe<Item, Err> + 'a> = Box::new(subscribe);
    let subscriber = Subscriber::from_subscribe(subscribe);

    let subscription = subscriber.clone_subscription();
    ((self.0).0)(subscriber);
    subscription
  }
}

impl<O, Item, Err, S> RawSubscribable<Item, Err, S>
  for ForkObservable<SharedObservable<O>>
where
  Item: 'static,
  Err: 'static,
  S: Subscribe<Item, Err> + Send + Sync + 'static,
  O: ObservableDispatch<
    Item,
    Err,
    Subscriber<Box<dyn Subscribe<Item, Err> + Send + Sync>, SharedSubscription>,
  >,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    let subscribe: Box<dyn Subscribe<Item, Err> + Send + Sync> =
      Box::new(subscribe);
    let subscriber = Subscriber::from_subscribe(subscribe).to_shared();
    let subscription = subscriber.clone_subscription();
    self.0.dispatch(subscriber);
    subscription
  }
}

impl<O, Item, Err, S> RawSubscribable<Item, Err, S>
  for ForkObservable<ForkObservable<O>>
where
  S: Subscribe<Item, Err>,
  ForkObservable<O>: RawSubscribable<Item, Err, S>,
{
  type Unsub = <ForkObservable<O> as RawSubscribable<Item, Err, S>>::Unsub;

  #[inline(always)]
  fn raw_subscribe(self, subscribe: S) -> Self::Unsub {
    self.0.raw_subscribe(subscribe)
  }
}

#[cfg(test)]
mod test {
  use crate::ops::Fork;
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
    let o = Observable::new(|subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.next(&4);
    });
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
  fn fork_and_share() {
    let observable = observable::empty!();
    // shared after fork
    observable.fork().to_shared().subscribe(|_| {});
    observable.fork().to_shared().subscribe(|_| {});

    // shared before fork
    let observable = observable::empty!().to_shared();
    observable.fork().subscribe(|_| {});
    observable.fork().subscribe(|_| {});
  }
}
