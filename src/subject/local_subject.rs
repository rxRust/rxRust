use crate::prelude::*;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

type LocalPublishers<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

pub type LocalSubject<'a, Item, Err> =
  Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>;

impl<'a, Item, Err> Debug for LocalSubject<'a, Item, Err> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field("observer_count", &self.observers.borrow().len())
      .finish()
  }
}

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

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  pub fn subscribed_size(&self) -> usize { self.observers.borrow_mut().len() }
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

    assert_eq!(subject.subscribed_size(), 1);
  }
  assert_eq!(test_code, 2);
}
