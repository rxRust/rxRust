use crate::{subscription::Subscription, Observer};
use std::marker::PhantomData;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<'a, Item, E, ON> {
  on_next: ON,
  on_err: Option<Box<dyn Fn(&E) + 'a>>,
  on_complete: Option<Box<dyn Fn() + 'a>>,
  _v: PhantomData<Item>,
}

impl<'a, Item, Err, ON> Subscriber<'a, Item, Err, ON>
where
  ON: Fn(&Item),
{
  pub fn new(next: ON) -> Self {
    Subscriber {
      on_next: next,
      on_err: None,
      on_complete: None,
      _v: PhantomData,
    }
  }
}

impl<'a, Item, Err, ON> Observer for Subscriber<'a, Item, Err, ON>
where
  ON: Fn(&Item),
{
  type Item = Item;
  type Err = Err;

  fn next(&self, v: &Self::Item) -> &Self {
    (self.on_next)(v);
    self
  }

  fn complete(self) {
    if let Some(ref comp) = self.on_complete {
      comp();
    }
  }

  fn error(self, err: &Self::Err) {
    if let Some(ref on_err) = self.on_err {
      on_err(err);
    }
  }
}

impl<'a, Item, Err, ON> Subscription<'a> for Subscriber<'a, Item, Err, ON> {
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

  fn unsubscribe(self) {
    // take the owner, and nobody can use after this.
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      let mut subscriber = Subscriber::new(|_v: &i32| {
        $next.set($next.get() + 1);
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

    let subscriber = create_subscriber!(next, err, complete);

    subscriber.next(&1).next(&2);
    assert_eq!(next.get(), 2);

    subscriber.complete();
    assert_eq!(complete.get(), 1);
  }

  #[test]
  fn next_and_error() {
    let next = Cell::new(0);
    let err = Cell::new(0);
    let complete = Cell::new(0);

    let subscriber = create_subscriber!(next, err, complete);

    subscriber.next(&1).next(&2);
    assert_eq!(next.get(), 2);

    subscriber.error(&());
    assert_eq!(err.get(), 1);
  }
}
