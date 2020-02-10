use crate::prelude::*;
use std::marker::PhantomData;

unsafe impl<F, Item, Err> Send for FnEmitter<F, Item, Err> where F: Send {}
unsafe impl<F, Item, Err> Sync for FnEmitter<F, Item, Err> where F: Sync {}
impl<F, Item, Err> Clone for FnEmitter<F, Item, Err>
where
  F: Clone,
{
  fn clone(&self) -> Self { FnEmitter(self.0.clone(), PhantomData) }
}

/// param `subscribe`: the function that is called when the Observable is
/// initially subscribed to. This function is given a Subscriber, to which
/// new values can be `next`ed, or an `error` method can be called to raise
/// an error, or `complete` can be called to notify of a successful
/// completion.
pub fn create<F, O, U, Item, Err>(
  subscribe: F,
) -> ObservableBase<FnEmitter<F, Item, Err>>
where
  F: FnOnce(Subscriber<O, U>),
  O: Observer<Item, Err>,
  U: SubscriptionLike,
{
  ObservableBase::new(FnEmitter(subscribe, PhantomData))
}

pub struct FnEmitter<F, Item, Err>(F, PhantomData<(Item, Err)>);

impl<'a, F, Item, Err> Emitter<'a> for FnEmitter<F, Item, Err>
where
  F: FnOnce(
    Subscriber<
      Box<dyn Observer<Item, Err> + 'a>,
      Box<dyn SubscriptionLike + 'a>,
    >,
  ),
{
  type Item = Item;
  type Err = Err;
  fn emit<O, U>(self, subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err> + 'a,
    U: SubscriptionLike + 'a,
  {
    (self.0)(Subscriber {
      observer: Box::new(subscriber.observer),
      subscription: Box::new(subscriber.subscription),
    })
  }
}

impl<F, Item, Err> SharedEmitter for FnEmitter<F, Item, Err>
where
  F: FnOnce(
    Subscriber<
      Box<dyn Observer<Item, Err> + Send + Sync + 'static>,
      SharedSubscription,
    >,
  ),
{
  type Item = Item;
  type Err = Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    (self.0)(Subscriber {
      observer: Box::new(subscriber.observer),
      subscription: subscriber.subscription,
    })
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

    observable::create(
      |mut subscriber: Subscriber<
        Box<dyn Observer<_, _> + Send + Sync + 'static>,
        SharedSubscription,
      >| {
        subscriber.next(&1);
        subscriber.next(&2);
        subscriber.next(&3);
        subscriber.complete();
        subscriber.next(&3);
        subscriber.error("never dispatch error");
      },
    )
    .shared()
    .subscribe_all(
      move |_| *next.lock().unwrap() += 1,
      move |_: &str| *err.lock().unwrap() += 1,
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
    let observable = observable::create(|_| {});
    // shared after fork
    observable.fork().shared().subscribe(|v: i32| {});
    observable.fork().shared().subscribe(|_| {});

    // shared before fork
    let observable = observable::create(|_| {}).shared();
    observable.fork().subscribe(|_: i32| {});
    observable.fork().subscribe(|_| {});
  }
}
