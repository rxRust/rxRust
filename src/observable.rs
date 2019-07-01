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

impl<'a, F, Item: 'a, Err: 'a> Subscribable<'a> for Observable<'a, F, Item, Err>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>) + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsubscribable = ObservableSubscription<'a, Item, Err, F>;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    let subscriber = Subscriber::new(next);
    ObservableSubscription {
      subscribe: Some(self.subscribe),
      subscriber,
    }
  }
}

pub struct ObservableSubscription<'a, Item, Err, F>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>),
{
  subscriber: Subscriber<'a, Item, Err>,
  subscribe: Option<F>,
}

impl<'a, F, Item, Err> Subscription<'a>
  for ObservableSubscription<'a, Item, Err, F>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>),
{
  type Err = Err;
  fn on_error<E>(&mut self, err: E) -> &mut Self
  where
    E: Fn(&Self::Err) + 'a,
  {
    self.subscriber.on_error(err);
    self
  }
  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a,
  {
    self.subscriber.on_complete(complete);;
    self
  }

  fn unsubscribe(&mut self) { self.subscriber.unsubscribe(); }
}

impl<'a, F, Item, Err> Drop for ObservableSubscription<'a, Item, Err, F>
where
  F: FnOnce(&mut Subscriber<'a, Item, Err>),
{
  fn drop(&mut self) { (self.subscribe.take().unwrap())(&mut self.subscriber); }
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
    .subscribe(|_| {
      next.set(next.get() + 1);
    })
    .on_complete(|| {
      complete.set(complete.get() + 1);
    })
    .on_error(|_: &&str| err.set(err.get() + 1));

    assert_eq!(next.get(), 3);
    assert_eq!(complete.get(), 1);
    assert_eq!(err.get(), 0);
  }
}
