use crate::{subscribable::OState, subscription::Subscription, Observer};
use std::marker::PhantomData;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<'a, Item, E> {
  stopped: bool,
  on_next: Box<dyn Fn(&Item) -> OState<E> + 'a>,
  on_err: Option<Box<dyn Fn(&E) + 'a>>,
  on_complete: Option<Box<dyn Fn() + 'a>>,
  _v: PhantomData<Item>,
}

impl<'a, Item, Err> Subscriber<'a, Item, Err> {
  pub fn new<ON>(next: ON) -> Self
  where
    ON: Fn(&Item) -> OState<Err> + 'a,
  {
    Subscriber {
      stopped: false,
      on_next: Box::new(next),
      on_err: None,
      on_complete: None,
      _v: PhantomData,
    }
  }
}

impl<'a, Item, Err> Observer for Subscriber<'a, Item, Err> {
  type Item = Item;
  type Err = Err;

  fn next(&self, v: &Self::Item) -> &Self {
    if !self.stopped {
      (self.on_next)(v);
    }
    self
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
}

impl<'a, Item, Err> Subscription<'a> for Subscriber<'a, Item, Err> {
  type Err = Err;
  fn on_error<E>(&mut self, on_err: E) -> &mut Self
  where
    E: Fn(&Self::Err) + 'a,
  {
    if self.on_err.is_none() {
      self.on_err.replace(Box::new(on_err));
    } else {
      panic!("Subscriber can only add only once `on_error` callback")
    }
    self
  }
  fn on_complete<C>(&mut self, on_comp: C) -> &mut Self
  where
    C: Fn() + 'a,
  {
    if self.on_complete.is_none() {
      self.on_complete.replace(Box::new(on_comp));
    } else {
      panic!("Subscriber can only add only once `on_error` callback")
    }
    self
  }

  fn unsubscribe(&mut self) { self.stopped = true; }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      let mut subscriber = Subscriber::new(|_v: &i32| {
        $next.set($next.get() + 1);
        OState::Next
      });
      subscriber
        .on_error(|_: &()| {
          $err.set($err.get() + 1);
        })
        .on_complete(|| {
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

    subscriber.next(&1).next(&2);
    subscriber.complete();
    subscriber.next(&3).next(&4);
    assert_eq!(next.get(), 2);
    assert_eq!(complete.get(), 1);
  }

  #[test]
  fn next_and_error() {
    let next = Cell::new(0);
    let err = Cell::new(0);
    let complete = Cell::new(0);

    let mut subscriber = create_subscriber!(next, err, complete);

    subscriber.next(&1).next(&2);
    subscriber.error(&());
    subscriber.next(&3).next(&4);

    assert_eq!(next.get(), 2);
    assert_eq!(err.get(), 1);
  }
}
