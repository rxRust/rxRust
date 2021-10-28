use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

type RcPublishers<P> = Rc<RefCell<SubjectObserver<Box<P>>>>;
type _LocalSubject<P> = Subject<RcPublishers<P>, LocalSubscription>;
type _LocalBehaviorSubject<P, Item> =
  BehaviorSubject<RcPublishers<P>, LocalSubscription, Item>;

pub type LocalSubject<'a, Item, Err> =
  _LocalSubject<dyn Publisher<Item = Item, Err = Err> + 'a>;

pub type LocalSubjectRef<'a, Item, Err> =
  _LocalSubject<dyn Publisher<Item = &'a Item, Err = Err> + 'a>;

pub type LocalSubjectErrRef<'a, Item, Err> =
  _LocalSubject<dyn Publisher<Item = Item, Err = &'a Err> + 'a>;

pub type LocalSubjectRefAll<'a, Item, Err> =
  _LocalSubject<dyn Publisher<Item = &'a Item, Err = &'a Err> + 'a>;

pub type LocalBehaviorSubject<'a, Item, Err> =
  _LocalBehaviorSubject<dyn Publisher<Item = Item, Err = Err> + 'a, Item>;

pub type LocalBehaviorSubjectRef<'a, Item, Err> =
  _LocalBehaviorSubject<dyn Publisher<Item = &'a Item, Err = Err> + 'a, Item>;

pub type LocalBehaviorSubjectErrRef<'a, Item, Err> =
  _LocalBehaviorSubject<dyn Publisher<Item = Item, Err = &'a Err> + 'a, Item>;

pub type LocalBehaviorSubjectRefAll<'a, Item, Err> = _LocalBehaviorSubject<
  dyn Publisher<Item = &'a Item, Err = &'a Err> + 'a,
  Item,
>;

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  #[inline]
  pub fn new() -> Self
  where
    Self: Default,
  {
    Self::default()
  }
  #[inline]
  pub fn subscribed_size(&self) -> usize { self.observers.borrow().len() }
}

impl<'a, Item, Err> Observable for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalObservable<'a> for LocalSubject<'a, Item, Err> {
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Item = Self::Item, Err = Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> LocalSubscription {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.borrow_mut().push(Box::new(subscriber));
    subscription
  }
}

impl<'a, Item, Err> LocalBehaviorSubject<'a, Item, Err> {
  #[inline]
  pub fn new(value: Item) -> Self
  where
    Self: Default,
  {
    LocalBehaviorSubject {
      subject: Default::default(),
      value,
    }
  }
  #[inline]
  pub fn subscribed_size(&self) -> usize {
    self.subject.observers.borrow().len()
  }
}

impl<'a, Item, Err> Observable for LocalBehaviorSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalObservable<'a>
  for LocalBehaviorSubject<'a, Item, Err>
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Item = Self::Item, Err = Self::Err> + 'a>(
    self,
    mut subscriber: Subscriber<O, LocalSubscription>,
  ) -> LocalSubscription {
    if !self.subject.is_closed() {
      subscriber.observer.next(self.value);
    }
    self.subject.actual_subscribe(subscriber)
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

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

  #[test]
  fn emit_ref() {
    let mut check = 0;

    {
      let mut subject = LocalSubjectRef::new();
      subject.clone().subscribe(|v| {
        check = *v;
      });
      subject.next(&1);
    }
    assert_eq!(check, 1);

    {
      let mut subject = LocalSubjectErrRef::new();
      subject
        .clone()
        .subscribe_err(|_: ()| {}, |err| check = *err);
      subject.error(&2);
    }
    assert_eq!(check, 2);

    {
      let mut subject = LocalSubjectRefAll::new();
      subject.clone().subscribe_err(|v| check = *v, |_: &()| {});
      subject.next(&1);
    }
    assert_eq!(check, 1);

    {
      let mut subject = LocalSubjectRefAll::new();
      subject
        .clone()
        .subscribe_err(|_: &()| {}, |err| check = *err);
      subject.error(&2);
    }
    assert_eq!(check, 2);
  }
}
