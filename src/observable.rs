use crate::prelude::*;
use std::marker::PhantomData;

pub struct Observable<'a, F, Item, Err>
where
  F: Fn(&mut Subscriber<'a, Item, Err>),
{
  subscribe: F,
  _a: PhantomData<&'a ()>,
  _v: PhantomData<Item>,
  _e: PhantomData<Err>,
}

impl<'a, F, Item, Err> Observable<'a, F, Item, Err>
where
  F: Fn(&mut Subscriber<'a, Item, Err>),
{
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
  F: Fn(&mut Subscriber<'a, Item, Err>) + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsubscribable = ObservableSubscription<'a, Item, Err, F>;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    // (self.subscribe)(&mut subscriber);
    let subscriber = Subscriber::new(next);
    ObservableSubscription {
      subscribe: self.subscribe,
      subscriber,
    }
  }
}

pub struct ObservableSubscription<'a, Item, Err, F>
where
  F: Fn(&mut Subscriber<'a, Item, Err>),
{
  subscriber: Subscriber<'a, Item, Err>,
  subscribe: F,
}

impl<'a, F, Item, Err> Subscription<'a>
  for ObservableSubscription<'a, Item, Err, F>
where
  F: Fn(&mut Subscriber<'a, Item, Err>),
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
  F: Fn(&mut Subscriber<'a, Item, Err>),
{
  fn drop(&mut self) { (self.subscribe)(&mut self.subscriber); }
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
