use crate::prelude::*;
use std::marker::PhantomData;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<Item, Err, Sub> {
  stopped: bool,
  subscribe: Sub,
  _v: PhantomData<(Item, Err)>,
}

unsafe impl<Item, Err, Sub> Send for Subscriber<Item, Err, Sub> where Sub: Send {}

unsafe impl<Item, Err, Sub> Sync for Subscriber<Item, Err, Sub> where Sub: Send {}

impl<Item, Err, Sub> Subscriber<Item, Err, Sub> {
  pub fn new(subscribe: Sub) -> Self {
    Subscriber {
      stopped: false,
      subscribe,
      _v: PhantomData,
    }
  }
}

impl<Item, Err, Sub> Observer for Subscriber<Item, Err, Sub>
where
  Sub: RxFn(RxValue<'_, Item, Err>) -> RxReturn<Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&self, v: &Self::Item) -> RxReturn<Self::Err> {
    if !self.stopped {
      self.subscribe.call((RxValue::Next(v),))
    } else {
      RxReturn::Continue
    }
  }

  fn complete(&mut self) {
    if self.stopped {
      return;
    }
    self.stopped = true;
    self.subscribe.call((RxValue::Complete,));
  }

  fn error(&mut self, err: &Self::Err) {
    if self.stopped {
      return;
    }
    self.stopped = true;
    self.subscribe.call((RxValue::Err(err),));
  }

  fn is_stopped(&self) -> bool { self.stopped }
}

impl<Item, Err, Sub> Subscription for Subscriber<Item, Err, Sub> {
  fn unsubscribe(&mut self) { self.stopped = true; }
}

impl<Item, Err, Sub> Publisher for Subscriber<Item, Err, Sub> where
  Sub: RxFn(RxValue<'_, Item, Err>) -> RxReturn<Err>
{
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      Subscriber::new(RxFnWrapper::new(|v: RxValue<'_, _, ()>| {
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
