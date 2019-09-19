use crate::prelude::*;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<S, U> {
  pub(crate) subscription: U,
  pub(crate) subscribe: S,
}

impl<S> Subscriber<S, LocalSubscription> {
  pub fn new(subscribe: S) -> Self {
    Subscriber {
      subscribe,
      subscription: LocalSubscription::default(),
    }
  }
}

impl<S, U> IntoShared for Subscriber<S, U>
where
  S: IntoShared,
  U: IntoShared,
{
  type Shared = Subscriber<S::Shared, U::Shared>;
  fn to_shared(self) -> Self::Shared {
    Subscriber {
      subscription: self.subscription.to_shared(),
      subscribe: self.subscribe.to_shared(),
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
      self.subscribe.on_next(v)
    }
  }

  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      self.subscribe.on_complete();
      self.subscription.unsubscribe()
    }
  }

  fn error(&mut self, err: &Err) {
    if !self.subscription.is_closed() {
      self.subscribe.on_error(err);
      self.subscription.unsubscribe();
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
      Subscriber::new(SubscribeAll::new(
        move |_: &_| *next.lock().unwrap() += 1,
        move |_: &_| *err.lock().unwrap() += 1,
        move || *complete.lock().unwrap() += 1,
      ))
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
      Subscriber::new(SubscribeAll::new(
        move |_: &_| next.set(next.get() + 1),
        move |_: &_| err.set(err.get() + 1),
        move || complete.set(complete.get() + 1),
      )),
    )
  }
}
