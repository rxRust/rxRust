use crate::{subscribable::OState, subscription::Subscription, Observer};
use std::marker::PhantomData;

/// Implements the Observer trait and Subscription trait. While the Observer is
/// the public API for consuming the values of an Observable, all Observers get
/// converted to a Subscriber, in order to provide Subscription capabilities.
///
pub struct Subscriber<'a, Item, E> {
  stopped: bool,
  // todo: should unbox the closure when rust support return impl trait in
  // trait method
  on_next: Box<dyn Fn(&Item) -> OState<E> + 'a>,
  on_err: Option<Box<dyn Fn(&E) + 'a>>,
  on_complete: Option<Box<dyn Fn() + 'a>>,
  _v: PhantomData<Item>,
}

impl<'a, Item, E> Subscriber<'a, Item, E> {
  pub fn new(
    next: impl Fn(&Item) -> OState<E> + 'a,
    error: Option<impl Fn(&E) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self {
    Subscriber {
      stopped: false,
      on_next: Box::new(next),
      on_err: error.map(|e| Box::new(e) as Box<dyn Fn(&E)>),
      on_complete: complete.map(|c| Box::new(c) as Box<dyn Fn() + 'a>),
      _v: PhantomData,
    }
  }

  pub fn is_stopped(&self) -> bool { self.stopped }
}

impl<'a, Item, Err> Observer for Subscriber<'a, Item, Err> {
  type Item = Item;
  type Err = Err;

  fn next(&self, v: &Self::Item) {
    if !self.stopped {
      (self.on_next)(v);
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
}

impl<'a, Item, Err> Subscription for Subscriber<'a, Item, Err> {
  fn unsubscribe(&mut self) { self.stopped = true; }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  macro_rules! create_subscriber {
    ($next:ident, $err: ident, $complete: ident) => {{
      Subscriber::new(
        |_v: &i32| {
          $next.set($next.get() + 1);
          OState::Next
        },
        Some(|_: &()| {
          $err.set($err.get() + 1);
        }),
        Some(|| {
          $complete.set($complete.get() + 1);
        }),
      )
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
