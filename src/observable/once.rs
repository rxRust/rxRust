use crate::prelude::*;
use std::marker::PhantomData;

/// Create an observable which can only subscribe once time.
/// `ObservableOnce` and its downstream can't be fork, but you can use
/// [`Subscribable::into_subject`] convert it to a [`Subject`]
pub struct ObservableOnce<F, Item, Err> {
  subscribe: F,
  _p: PhantomData<(Item, Err)>,
}

impl<F, Item, Err> ObservableOnce<F, Item, Err> {
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
}

impl<F, Item, Err> RawSubscribable for ObservableOnce<F, Item, Err>
where
  F: FnOnce(Box<dyn Observer<Item, Err> + Send>),
{
  type Item = Item;
  type Err = Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let subscriber = Subscriber::new(subscribe);

    let subscription = subscriber.clone_subscription();
    (self.subscribe)(Box::new(subscriber));
    Box::new(subscription)
  }
}

#[inline(always)]
pub fn once<F, Item, Err>(f: F) -> ObservableOnce<F, Item, Err>
where
  F: FnOnce(Box<dyn Observer<Item, Err> + Send>),
{
  ObservableOnce::new(f)
}

#[test]
#[should_panic(expected = "subscribe hit!")]
fn smoke() {
  once::<_, _, ()>(move |observer| {
    observer.next(&1);
  })
  .subscribe(|v| {
    assert_eq!(*v, 1);
    panic!("subscribe hit!");
  });
}
