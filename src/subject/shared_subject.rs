use crate::prelude::*;
use std::sync::{Arc, Mutex};

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item = Item, Err = Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

impl<Item, Err> SharedSubject<Item, Err> {
  #[inline]
  pub fn new() -> Self
  where
    Self: Default,
  {
    Self::default()
  }
  #[inline]
  pub fn subscribed_size(&self) -> usize {
    self.observers.observers.lock().unwrap().len()
  }
}

impl<Item, Err> Observable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self
      .observers
      .observers
      .lock()
      .unwrap()
      .push(Box::new(subscriber));
    subscription
  }
}

#[test]
fn smoke() {
  let test_code = Arc::new(Mutex::new("".to_owned()));
  let mut subject = SharedSubject::new();
  let c_test_code = test_code.clone();
  subject.clone().into_shared().subscribe(move |v: &str| {
    *c_test_code.lock().unwrap() = v.to_owned();
  });
  subject.next("test shared subject");
  assert_eq!(*test_code.lock().unwrap(), "test shared subject");
  assert_eq!(subject.subscribed_size(), 1);
}
