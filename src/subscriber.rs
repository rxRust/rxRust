use crate::prelude::*;
use std::marker::PhantomData;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<Item, Err, ON, OE, OC> {
  stopped: bool,
  on_next: ON,
  on_err: Option<OE>,
  on_complete: Option<OC>,
  _v: PhantomData<(Item, Err)>,
}

unsafe impl<Item, Err, ON, OE, OC> Send for Subscriber<Item, Err, ON, OE, OC>
where
  ON: Send,
  OE: Send,
  OC: Send,
{
}

unsafe impl<Item, Err, ON, OE, OC> Sync for Subscriber<Item, Err, ON, OE, OC>
where
  ON: Send,
  OE: Send,
  OC: Send,
{
}

impl<Item, Err, ON, OE, OC> Subscriber<Item, Err, ON, OE, OC> {
  pub fn new(on_next: ON) -> Self {
    Subscriber {
      stopped: false,
      on_next,
      on_err: None,
      on_complete: None,
      _v: PhantomData,
    }
  }

  pub fn on_error(&mut self, on_err: OE) {
    if self.on_err.is_none() {
      self.on_err.replace(on_err);
    } else {
      panic!("subscriber only accept on_error once");
    }
  }

  pub fn on_complete(&mut self, on_comp: OC) {
    if self.on_complete.is_none() {
      self.on_complete.replace(on_comp);
    } else {
      panic!("subscriber only accept on_error once");
    }
  }
}

impl<Item, Err, ON, OE, OC> Observer for Subscriber<Item, Err, ON, OE, OC>
where
  ON: Fn(&Item) -> RxReturn<Err>,
  OE: Fn(&Err),
  OC: Fn(),
{
  type Item = Item;
  type Err = Err;

  fn next(&self, v: &Self::Item) -> RxReturn<Self::Err> {
    if !self.stopped {
      (self.on_next)(v)
    } else {
      RxReturn::Continue
    }
  }

  fn complete(&mut self) {
    if self.stopped {
      return;
    } else {
      self.stopped = true;
    }
    if let Some(comp) = self.on_complete.take() {
      comp();
    }
  }

  fn error(&mut self, err: &Self::Err) {
    if self.stopped {
      return;
    } else {
      self.stopped = true;
    }
    if let Some(on_err) = self.on_err.take() {
      on_err(err);
    }
  }

  fn is_stopped(&self) -> bool { self.stopped }
}

impl<Item, Err, ON, OE, OC> Subscription for Subscriber<Item, Err, ON, OE, OC> {
  fn unsubscribe(&mut self) { self.stopped = true; }
}

impl<Item, Err, ON, OE, OC> Publisher for Subscriber<Item, Err, ON, OE, OC>
where
  ON: Fn(&Item) -> RxReturn<Err>,
  OE: Fn(&Err),
  OC: Fn(),
{
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      let mut subscriber = Subscriber::new(|_v: &i32| {
        $next.set($next.get() + 1);
        RxReturn::Continue
      });
      subscriber.on_error(|_: &()| {
        $err.set($err.get() + 1);
      });
      subscriber.on_complete(|| {
        $complete.set($complete.get() + 1);
      });
      subscriber
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
