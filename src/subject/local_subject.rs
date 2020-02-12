use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

type LocalPublishers<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

pub type LocalSubject<'a, Item, Err> =
  Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>;

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  pub fn local() -> Self {
    LocalSubject {
      observers: Rc::new(RefCell::new(vec![])),
      subscription: LocalSubscription::default(),
    }
  }
}

impl<'a, Item, Err> Observable<'a> for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
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
    let mut subject = LocalSubject::local();
    subject.clone().subscribe(|v| {
      test_code = v;
    });
    subject.next(2);
  }
  assert_eq!(test_code, 2);
}
