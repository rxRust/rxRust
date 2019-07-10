use crate::prelude::*;
use std::marker::PhantomData;

mod from_iter;
pub use from_iter::from_iter;

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rx_rs
///
pub struct Observable<'a, F, Item, Err>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>),
{
  subscribe: F,
  _a: PhantomData<&'a ()>,
  _v: PhantomData<Item>,
  _e: PhantomData<Err>,
}

impl<'a, F, Item, Err> Observable<'a, F, Item, Err>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>),
{
  /// param `subscribe`: the function that is called when the Observable is
  /// initially subscribed to. This function is given a Subscriber, to which
  /// new values can be `next`ed, or an `error` method can be called to raise
  /// an error, or `complete` can be called to notify of a successful
  /// completion.
  pub fn new(subscribe: F) -> Self {
    Self {
      subscribe,
      _a: PhantomData,
      _v: PhantomData,
      _e: PhantomData,
    }
  }
}

impl<'a, F, Item: 'a, Err: 'a> ImplSubscribable<'a>
  for Observable<'a, F, Item, Err>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>) + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsubscribable = Subscriber<'a, Item, Err>;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsubscribable {
    let mut subscriber = Subscriber::new(next, error, complete);
    (self.subscribe)(&mut subscriber);
    subscriber
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn proxy_call() {
    let next = Cell::new(0);
    let err = Cell::new(0);
    let complete = Cell::new(0);

    Observable::new(|subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error(&"never dispatch error");
    })
    .subscribe_err_complete(
      |_| next.set(next.get() + 1),
      |_: &&str| err.set(err.get() + 1),
      || complete.set(complete.get() + 1),
    );

    assert_eq!(next.get(), 3);
    assert_eq!(complete.get(), 1);
    assert_eq!(err.get(), 0);
  }
}
