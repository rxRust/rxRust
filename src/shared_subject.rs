use crate::prelude::*;
use observer::observer_proxy_impl;
use std::sync::{Arc, Mutex};

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

#[derive(Clone)]
pub struct SharedSubject<Item, Err> {
  observers: SharedPublishers<Item, Err>,
  subscription: SharedSubscription,
}

subscription_proxy_impl!(SharedSubject<Item, Err>, {subscription}, <Item, Err>);
observer_proxy_impl!(SharedSubject<Item, Err>, {observers}, Item, Err, {where Item: Copy, Err: Copy});

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;

  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    mut self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.lock().unwrap().push(Box::new(subscriber));
    subscription
  }
}

impl<Item, Err> SharedSubject<Item, Err> {
  pub fn shared() -> Self {
    SharedSubject {
      observers: Arc::new(Mutex::new(vec![])),
      subscription: SharedSubscription::default(),
    }
  }
}

#[test]

fn smoke() {
  let test_code = Arc::new(Mutex::new("".to_owned()));
  let subject = SharedSubject::shared();
  subject.shared().clone().subscribe(|v: &str| {
    *test_code.lock().unwrap() = v.to_owned();
  });
  subject.next("test shared subject");
  assert_eq!(*test_code.lock().unwrap(), "test shared subject");
}
