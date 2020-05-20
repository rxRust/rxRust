use crate::prelude::*;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

impl<Item, Err> Debug for SharedSubject<Item, Err> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SharedSubject")
      .field(
        "observer_count",
        &self
          .observers
          .lock()
          .expect("error obtaining observer lock")
          .len(),
      )
      .finish()
  }
}

impl<Item, Err> Observable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    mut self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.lock().unwrap().push(Box::new(subscriber));
    subscription
  }
}

impl<Item, Err> SharedSubject<Item, Err> {
  pub fn subscribed_size(&self) -> usize {
    self.observers.lock().unwrap().len()
  }
}
#[test]

fn smoke() {
  let test_code = Arc::new(Mutex::new("".to_owned()));
  let mut subject = SharedSubject::new();
  let c_test_code = test_code.clone();
  subject.clone().to_shared().subscribe(move |v: &str| {
    *c_test_code.lock().unwrap() = v.to_owned();
  });
  subject.next("test shared subject");
  assert_eq!(*test_code.lock().unwrap(), "test shared subject");
  assert_eq!(subject.subscribed_size(), 1);
}
