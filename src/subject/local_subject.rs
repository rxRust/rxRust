use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

type LocalPublishers<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

pub type LocalSubject<'a, Item, Err> =
  Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>;

impl<'a, Item, Err> Observable for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}
impl<'a, Item, Err> LocalObservable<'a> for LocalSubject<'a, Item, Err> {
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    mut self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> LocalSubscription {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.borrow_mut().push(Box::new(subscriber));
    subscription
  }
}

#[test]
fn smoke() {
  let mut test_code = 1;
  {
    let mut subject = LocalSubject::new();
    subject.clone().subscribe(|v| {
      test_code = v;
    });
    subject.next(2);
  }
  assert_eq!(test_code, 2);
}
