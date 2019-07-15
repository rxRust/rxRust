use crate::prelude::*;
use std::marker::PhantomData;

mod from_iter;
pub use from_iter::from_iter;

/// A representation of any set of values over any amount of time. This is the
/// most basic building block rx_rs
///
pub struct Observable<F, Item, Err> {
  subscribe: F,
  _p: PhantomData<(Item, Err)>,
}

impl<'a, F, Item, Err> Observable<F, Item, Err>
where
  F: Fn(&mut Subscriber<'a, Item, Err>),
{
  /// param `subscribe`: the function that is called when the Observable is
  /// initially subscribed to. This function is given a Subscriber, to which
  /// new values can be `next`ed, or an `error` method can be called to raise
  /// an error, or `complete` can be called to notify of a successful
  /// completion.
  pub fn new(subscribe: F) -> Self {
    Self {
      subscribe,
      _p: PhantomData,
    }
  }

  #[inline]
  fn consume(
    &self,
    next: impl Fn(&Item) -> OState<Err> + 'a,
    error: Option<impl Fn(&Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Subscriber<'a, Item, Err> {
    let mut subscriber = Subscriber::new(next);
    if error.is_some() {
      subscriber.on_error(error.unwrap())
    };
    if complete.is_some() {
      subscriber.on_complete(complete.unwrap())
    };
    (self.subscribe)(&mut subscriber);
    subscriber
  }
}

impl<'a, F, Item: 'a, Err: 'a> ImplSubscribable<'a> for Observable<F, Item, Err>
where
  F: Fn(&mut Subscriber<'a, Item, Err>) + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsub = Subscriber<'a, Item, Err>;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    self.consume(next, error, complete)
  }
}

impl<'a, F, Item: 'a, Err: 'a> ImplSubscribable<'a>
  for &'a Observable<F, Item, Err>
where
  F: Fn(&mut Subscriber<'a, Item, Err>) + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsub = Subscriber<'a, Item, Err>;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    self.consume(next, error, complete)
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

  #[test]
  fn support_ref_subscribe() {
    let o = Observable::new(|subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.next(&4);
      subscriber.error(&"");
    });
    let o1 = &o;
    let o2 = &o;
    let sum1 = Cell::new(0);
    let sum2 = Cell::new(0);
    o1.subscribe(|v| sum1.set(sum1.get() + v));
    o2.subscribe(|v| sum2.set(sum2.get() + v));

    assert_eq!(sum1.get(), 10);
    assert_eq!(sum2.get(), 10);
  }
}
