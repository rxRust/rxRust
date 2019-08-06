use crate::prelude::*;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

pub type SimpleSubscription = Arc<AtomicBool>;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<Sub> {
  stopped: SimpleSubscription,
  subscribe: Sub,
}

unsafe impl<Sub> Send for Subscriber<Sub> where Sub: Send {}

unsafe impl<Sub> Sync for Subscriber<Sub> where Sub: Send {}

impl<Sub> Subscriber<Sub> {
  pub fn new(subscribe: Sub) -> Self {
    Subscriber {
      stopped: Arc::new(AtomicBool::new(false)),
      subscribe,
    }
  }

  #[inline(always)]
  pub fn clone_subscription(&self) -> SimpleSubscription {
    self.stopped.clone()
  }
}

impl<Item, Err, Sub> Observer<Item, Err> for Subscriber<Sub>
where
  Sub: RxFn(RxValue<&'_ Item, &'_ Err>) -> RxReturn<Err>,
{
  fn next(&self, v: &Item) -> RxReturn<Err> {
    if !self.stopped.load(Ordering::Relaxed) {
      self.subscribe.call((RxValue::Next(v),))
    } else {
      RxReturn::Continue
    }
  }

  fn complete(&mut self) {
    if self.stopped.load(Ordering::Relaxed) {
      return;
    }
    self.stopped.store(true, Ordering::Relaxed);
    self.subscribe.call((RxValue::Complete,));
  }

  fn error(&mut self, err: &Err) {
    if self.stopped.load(Ordering::Relaxed) {
      return;
    }
    self.stopped.store(true, Ordering::Relaxed);
    self.subscribe.call((RxValue::Err(err),));
  }

  fn is_stopped(&self) -> bool { self.stopped.load(Ordering::Relaxed) }
}

impl Subscription for SimpleSubscription {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.store(true, Ordering::Relaxed); }
}

impl<Sub> Subscription for Subscriber<Sub> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.stopped.unsubscribe(); }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      Subscriber::new(RxFnWrapper::new(|v: RxValue<&'_ _, &'_ ()>| {
        match v {
          RxValue::Next(_) => $next.set($next.get() + 1),
          RxValue::Complete => $complete.set($complete.get() + 1),
          RxValue::Err(_) => $err.set($err.get() + 1),
        };
        RxReturn::Continue
      }))
    }};
  }

  #[test]
  fn next_and_complete() {
    let next = Cell::new(0);
    let err = Cell::new(0);
    let complete = Cell::new(0);

    let mut subscriber = create_subscriber!(next, err, complete);

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
    let next = Cell::new(0);
    let err = Cell::new(0);
    let complete = Cell::new(0);

    let mut subscriber = create_subscriber!(next, err, complete);

    subscriber.next(&1);
    subscriber.next(&2);
    subscriber.error(&());
    subscriber.next(&3);
    subscriber.next(&4);

    assert_eq!(next.get(), 2);
    assert_eq!(err.get(), 1);
  }
}
