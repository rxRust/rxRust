use crate::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<S, U> {
  stopped: U,
  subscribe: S,
}

impl<S> Subscriber<S, Rc<Cell<bool>>> {
  pub fn new(subscribe: S) -> Self {
    Subscriber {
      stopped: Rc::new(Cell::new(false)),
      subscribe,
    }
  }
}
impl<S, U> Subscriber<S, U>
where
  U: Clone,
{
  #[inline(always)]
  pub fn clone_subscription(&self) -> U { self.stopped.clone() }
}

impl<Item, Err, S, U> Observer<Item, Err> for Subscriber<S, U>
where
  S: Subscribe<Item, Err>,
  U: Subscription,
{
  fn next(&self, v: &Item) {
    if !self.stopped.is_stopped() {
      self.subscribe.run(RxValue::Next(v))
    }
  }

  fn complete(&mut self) {
    if !self.stopped.is_stopped() {
      self.subscribe.run(RxValue::Complete);
      self.stopped.unsubscribe()
    }
  }

  fn error(&mut self, err: &Err) {
    if !self.stopped.is_stopped() {
      self.stopped.unsubscribe();
      self.subscribe.run(RxValue::Err(err));
    }
  }

  fn is_stopped(&self) -> bool { self.stopped.is_stopped() }
}

impl<S, U, Item, Err> Subscribe<Item, Err> for Subscriber<S, U>
where
  S: Subscribe<Item, Err>,
{
  #[inline(always)]
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) { self.subscribe.run(v); }
}

impl<Item, Err, S>
  IntoSharedSubscribe<Item, Err, Subscriber<S, Arc<AtomicBool>>>
  for Subscriber<S, Rc<Cell<bool>>>
where
  S: Subscribe<Item, Err> + Send + Sync + 'static,
{
  fn to_shared(self) -> Subscriber<S, Arc<AtomicBool>> {
    Subscriber {
      stopped: Arc::new(AtomicBool::new(self.stopped.get())),
      subscribe: self.subscribe,
    }
  }
}

impl Subscription for Arc<AtomicBool> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.store(true, Ordering::Relaxed); }

  #[inline(always)]
  fn is_stopped(&self) -> bool { self.load(Ordering::Relaxed) }
}

impl Subscription for Rc<Cell<bool>> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.set(true) }

  #[inline(always)]
  fn is_stopped(&self) -> bool { self.get() }
}

impl<Sub, U> Subscription for Subscriber<Sub, U>
where
  U: Subscription,
{
  #[inline(always)]
  fn unsubscribe(&mut self) { self.stopped.unsubscribe(); }

  #[inline(always)]
  fn is_stopped(&self) -> bool { self.stopped.is_stopped() }
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
