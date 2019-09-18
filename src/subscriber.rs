use crate::prelude::*;

pub struct SubscribeWrapper<F>(F);

impl<Item, Err, F> Subscribe<Item, Err> for SubscribeWrapper<F>
where
  F: Fn(RxValue<&'_ Item, &'_ Err>),
{
  #[inline(always)]
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) { (self.0)(v) }
}

impl<Item, Err, F> IntoSharedSubscribe<Item, Err> for SubscribeWrapper<F>
where
  F: Fn(RxValue<&'_ Item, &'_ Err>) + Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<S, U> {
  pub(crate) subscription: U,
  pub(crate) subscribe: S,
}

impl<F> Subscriber<SubscribeWrapper<F>, LocalSubscription> {
  pub fn new(subscribe: F) -> Self {
    Subscriber {
      subscription: LocalSubscription::default(),
      subscribe: SubscribeWrapper(subscribe),
    }
  }
}

impl<S> Subscriber<S, LocalSubscription> {
  pub fn from_subscribe(subscribe: S) -> Self {
    Subscriber {
      subscribe,
      subscription: LocalSubscription::default(),
    }
  }
}

impl<Item, Err, S, U> Observer<Item, Err> for Subscriber<S, U>
where
  S: Subscribe<Item, Err>,
  U: SubscriptionLike,
{
  fn next(&self, v: &Item) {
    if !self.subscription.is_closed() {
      self.subscribe.run(RxValue::Next(v))
    }
  }

  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      self.subscribe.run(RxValue::Complete);
      self.subscription.unsubscribe()
    }
  }

  fn error(&mut self, err: &Err) {
    if !self.subscription.is_closed() {
      self.subscription.unsubscribe();
      self.subscribe.run(RxValue::Err(err));
    }
  }
}

impl<S, U, Item, Err> Subscribe<Item, Err> for Subscriber<S, U>
where
  S: Subscribe<Item, Err>,
{
  #[inline(always)]
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) { self.subscribe.run(v); }
}

impl<Item, Err, S> IntoSharedSubscribe<Item, Err>
  for Subscriber<S, LocalSubscription>
where
  S: IntoSharedSubscribe<Item, Err>,
{
  type Shared = Subscriber<S::Shared, SharedSubscription>;
  fn to_shared(self) -> Subscriber<S::Shared, SharedSubscription> {
    Subscriber {
      subscription: SharedSubscription::default(),
      subscribe: self.subscribe.to_shared(),
    }
  }
}

impl<Item, Err, S> IntoSharedSubscribe<Item, Err>
  for Subscriber<S, SharedSubscription>
where
  S: IntoSharedSubscribe<Item, Err>,
{
  type Shared = Subscriber<S::Shared, SharedSubscription>;
  fn to_shared(self) -> Subscriber<S::Shared, SharedSubscription> {
    Subscriber {
      subscription: self.subscription,
      subscribe: self.subscribe.to_shared(),
    }
  }
}

impl<Sub, U> SubscriptionLike for Subscriber<Sub, U>
where
  U: SubscriptionLike,
{
  #[inline(always)]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }

  #[inline(always)]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;
  use std::rc::Rc;
  use std::sync::{Arc, Mutex};

  #[test]
  fn shared_next_complete() {
    let (next, _, complete, mut subscriber) = shared_subscriber_creator();
    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.complete();
    subscriber.next(&3);
    subscriber.next(&4);
    assert_eq!(*next.lock().unwrap(), 2);
    assert_eq!(*complete.lock().unwrap(), 1);
  }

  #[test]
  fn shared_err_complete() {
    let (next, error, _, mut subscriber) = shared_subscriber_creator();
    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.error(&());
    subscriber.next(&3);
    subscriber.next(&4);

    assert_eq!(*next.lock().unwrap(), 2);
    assert_eq!(*error.lock().unwrap(), 1);
  }

  fn shared_subscriber_creator() -> (
    Arc<Mutex<i32>>,
    Arc<Mutex<i32>>,
    Arc<Mutex<i32>>,
    impl Observer<i32, ()>,
  ) {
    let next = Arc::new(Mutex::new(0));
    let err = Arc::new(Mutex::new(0));
    let complete = Arc::new(Mutex::new(0));

    (
      next.clone(),
      err.clone(),
      complete.clone(),
      Subscriber::new(move |v: RxValue<&'_ _, &'_ ()>| match v {
        RxValue::Next(_) => *next.lock().unwrap() += 1,
        RxValue::Err(_) => *err.lock().unwrap() += 1,
        RxValue::Complete => *complete.lock().unwrap() += 1,
      })
      .to_shared(),
    )
  }

  #[test]

  fn next_and_complete() {
    let (next, _, complete, mut subscriber) = subscriber_creator();

    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.complete();
    subscriber.next(&3);
    subscriber.next(&4);
    assert_eq!(next.get(), 2);
    assert_eq!(complete.get(), 1);
  }

  #[test]
  fn next_and_error() {
    let (next, error, _, mut subscriber) = subscriber_creator();

    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.error(&());
    subscriber.next(&3);
    subscriber.next(&4);

    assert_eq!(next.get(), 2);
    assert_eq!(error.get(), 1);
  }

  fn subscriber_creator() -> (
    Rc<Cell<i32>>,
    Rc<Cell<i32>>,
    Rc<Cell<i32>>,
    impl Observer<i32, ()>,
  ) {
    let next = Rc::new(Cell::new(0));
    let err = Rc::new(Cell::new(0));
    let complete = Rc::new(Cell::new(0));

    (
      next.clone(),
      err.clone(),
      complete.clone(),
      Subscriber::new(move |v: RxValue<&'_ _, &'_ ()>| match v {
        RxValue::Next(_) => next.set(next.get() + 1),
        RxValue::Err(_) => err.set(err.get() + 1),
        RxValue::Complete => complete.set(complete.get() + 1),
      }),
    )
  }
}
