use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct _LocalSubject<O: Observer>(
  Rc<RefCell<Subject<O, Rc<RefCell<SingleSubscription>>>>>,
);
pub struct _LocalBehaviorSubject<O: Observer>(
  Rc<RefCell<BehaviorSubject<O, Rc<RefCell<SingleSubscription>>>>>,
);

pub type LocalSubject<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = Item, Err = Err> + 'a>>;

pub type LocalSubjectRef<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = &'a Item, Err = Err> + 'a>>;

pub type LocalSubjectErrRef<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = Item, Err = &'a Err> + 'a>>;

pub type LocalSubjectRefAll<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = &'a Item, Err = &'a Err> + 'a>>;

pub type LocalBehaviorSubject<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = Item, Err = Err> + 'a>>;

pub type LocalBehaviorSubjectRef<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = &'a Item, Err = Err> + 'a>>;

pub type LocalBehaviorSubjectErrRef<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = Item, Err = &'a Err> + 'a>>;

pub type LocalBehaviorSubjectRefAll<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = &'a Item, Err = &'a Err> + 'a>>;

impl<'a, Item, Err> Observable for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalObservable<'a> for LocalSubject<'a, Item, Err> {
  type Unsub = Rc<RefCell<SingleSubscription>>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.0.borrow_mut().subscribe(Box::new(observer))
  }
}

impl<'a, Item, Err> Observable for LocalBehaviorSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item: Clone, Err> LocalObservable<'a>
  for LocalBehaviorSubject<'a, Item, Err>
{
  type Unsub = Rc<RefCell<SingleSubscription>>;
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    if !self.0.borrow().subject.is_closed() {
      observer.next(self.0.borrow_mut().value.clone());
    }
    self.0.borrow_mut().subject.subscribe(Box::new(observer))
  }
}

impl<O: Observer> _LocalSubject<O> {
  #[inline]
  pub fn new() -> Self { _LocalSubject(<_>::default()) }
}

impl<O: Observer> _LocalBehaviorSubject<O> {
  #[inline]
  pub fn new(value: O::Item) -> Self {
    _LocalBehaviorSubject(Rc::new(RefCell::new(BehaviorSubject {
      subject: <_>::default(),
      value,
    })))
  }
}

impl<O: Observer> Clone for _LocalSubject<O> {
  fn clone(&self) -> Self { _LocalSubject(self.0.clone()) }
}

impl<O: Observer> Clone for _LocalBehaviorSubject<O> {
  fn clone(&self) -> Self { _LocalBehaviorSubject(self.0.clone()) }
}

impl<O: Observer> Observer for _LocalSubject<O>
where
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;

  fn next(&mut self, v: Self::Item) { self.0.borrow_mut().next(v); }

  fn error(&mut self, err: Self::Err) { self.0.borrow_mut().error(err); }

  fn complete(&mut self) { self.0.borrow_mut().complete() }
}

impl<O: Observer> SubscriptionLike for _LocalSubject<O> {
  fn unsubscribe(&mut self) { self.0.borrow_mut().unsubscribe() }

  fn is_closed(&self) -> bool { self.0.borrow().is_closed() }
}

impl<O: Observer> TearDownSize for _LocalSubject<O> {
  fn teardown_size(&self) -> usize { self.0.borrow().observers.len() }
}
impl<O: Observer> Observer for _LocalBehaviorSubject<O>
where
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;

  fn next(&mut self, v: Self::Item) { self.0.borrow_mut().subject.next(v); }

  fn error(&mut self, err: Self::Err) {
    self.0.borrow_mut().subject.error(err);
  }

  fn complete(&mut self) { self.0.borrow_mut().subject.complete() }
}

impl<O: Observer> SubscriptionLike for _LocalBehaviorSubject<O> {
  fn unsubscribe(&mut self) { self.0.borrow_mut().unsubscribe() }

  fn is_closed(&self) -> bool { self.0.borrow().is_closed() }
}

impl<O: Observer> TearDownSize for _LocalBehaviorSubject<O> {
  #[inline]
  fn teardown_size(&self) -> usize { self.0.borrow().subject.subscribed_size() }
}

impl<O: Observer> Default for _LocalSubject<O> {
  #[inline]
  fn default() -> Self { Self(<_>::default()) }
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

      assert_eq!(subject.teardown_size(), 1);
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
