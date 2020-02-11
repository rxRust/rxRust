use crate::prelude::*;
use observer::observer_proxy_impl;
use std::cell::RefCell;
use std::rc::Rc;

type LocalSubjectObserver<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

#[derive(Clone)]
pub struct LocalSubject<'a, Item, Err> {
  observers: LocalSubjectObserver<'a, Item, Err>,
  subscription: LocalSubscription,
}

subscription_proxy_impl!(
  LocalSubject<'a, Item, Err>, {subscription}, <'a, Item, Err>);
observer_proxy_impl!(
  LocalSubject<'a, Item, Err>, {observers}, Item, Err, <'a>,
  {where Item: Copy, Err: Copy});

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
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + 'a,
    U: SubscriptionLike + Clone + 'static,
  >(
    mut self,
    subscriber: Subscriber<O, U>,
  ) -> U {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.borrow_mut().push(Box::new(subscriber));
    subscription
  }
}

#[test]
fn smoke() {
  let mut test_code = 1;
  let subject = LocalSubject::local();
  subject.clone().subscribe(|v| {
    test_code = 2;
  });
  subject.next(2);
  assert_eq!(test_code, 2);
}
