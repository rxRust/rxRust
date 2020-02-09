use crate::prelude::*;
use observer::{
  observer_complete_proxy_impl, observer_error_proxy_impl,
  observer_next_proxy_impl,
};
use std::marker::PhantomData;

/// A representation of any set of values over any amount of time.
pub struct ObservableFromFn<F, Item, Err>(F, PhantomData<(Item, Err)>);
pub struct ObservableFromFnShared<F, Item, Err>(F, PhantomData<(Item, Err)>);

pub struct PublisherFn<P>(P);
observer_next_proxy_impl!(PublisherFn<P>, P, 0, <Item, P>, Item);
observer_error_proxy_impl!(PublisherFn<P>, P, 0, <Err, P>, Err);
observer_complete_proxy_impl!(PublisherFn<P>, P, 0, <P>);

macro impl_observable($t: ident) {
  unsafe impl<F, Item, Err> Send for $t<F, Item, Err> {}
  unsafe impl<F, Item, Err> Sync for $t<F, Item, Err> {}
  impl<F, Item, Err> Clone for $t<F, Item, Err>
  where
    F: Clone,
  {
    fn clone(&self) -> Self { $t(self.0.clone(), PhantomData) }
  }
  impl<F, Item, Err> Fork for $t<F, Item, Err>
  where
    F: Clone,
  {
    type Output = Self;
    #[inline]
    fn fork(&self) -> Self::Output { self.clone() }
  }
}

impl_observable!(ObservableFromFn);
impl_observable!(ObservableFromFnShared);

/// param `subscribe`: the function that is called when the Observable is
/// initially subscribed to. This function is given a Subscriber, to which
/// new values can be `next`ed, or an `error` method can be called to raise
/// an error, or `complete` can be called to notify of a successful
/// completion.
pub fn create<F, P, Item, Err>(subscribe: F) -> ObservableFromFn<F, Item, Err>
where
  F: FnOnce(PublisherFn<P>),
  P: Publisher<Item, Err>,
{
  ObservableFromFn(subscribe, PhantomData)
}

#[derive(Clone)]
pub struct FnEmitter<F, Item, Err>(F, PhantomData<(Item, Err)>);

impl<'a, F, Item, Err> Emitter for FnEmitter<F, Item, Err>
where
  F: FnOnce(Box<dyn Publisher<Item, Err> + 'a>),
{
  type Item = Item;
  type Err = Err;
  fn emit<O, U>(self, subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err>,
    U: SubscriptionLike,
  {
    unimplemented!();
    // (self.0)(Box::new(subscriber))
  }
}

impl<'a, F, O, U, Item, Err> Observable<O, U> for ObservableFromFn<F, Item, Err>
where
  O: Observer<Item, Err> + 'a,
  U: SubscriptionLike + 'a,
  F: FnOnce(PublisherFn<Box<dyn Publisher<Item, Err> + 'a>>),
  U: SubscriptionLike + Clone,
{
  type Unsub = U;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    (self.0)(PublisherFn(Box::new(subscriber)));
    subscription
  }
}

impl<F, O, U, Item, Err> Observable<O, U>
  for ObservableFromFnShared<F, Item, Err>
where
  O: IntoShared,
  O::Shared: Observer<Item, Err> + Send + Sync + 'static,
  U: IntoShared,
  U::Shared: SubscriptionLike + Send + Sync + 'static,
  F: FnOnce(PublisherFn<Box<dyn Publisher<Item, Err> + Send + Sync>>),
  U: SubscriptionLike + Clone,
{
  type Unsub = U;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    (self.0)(PublisherFn(Box::new(subscriber.to_shared())));
    subscription
  }
}

impl<F, Item, Err> IntoShared for ObservableFromFn<F, Item, Err>
where
  F: Send + Sync + 'static,
  Item: 'static,
  Err: 'static,
{
  type Shared = ObservableFromFnShared<F, Item, Err>;
  #[inline]
  fn to_shared(self) -> Self::Shared {
    ObservableFromFnShared(self.0, PhantomData)
  }
}

impl<F, Item, Err> IntoShared for ObservableFromFnShared<F, Item, Err>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline]
  fn to_shared(self) -> Self::Shared { self }
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

    observable::create(|mut subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error("never dispatch error");
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
    let o = observable::create(|mut subscriber| {
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
    let observable = observable::create::<_, _, (), ()>(|_| {});
    // shared after fork
    observable.fork().to_shared().subscribe(|_| {});
    observable.fork().to_shared().subscribe(|_| {});

    // shared before fork
    let observable = observable::create::<_, _, (), ()>(|_| {}).to_shared();
    observable.fork().subscribe(|_| {});
    observable.fork().subscribe(|_| {});
  }
}
