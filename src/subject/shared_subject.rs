use crate::{behavior_subject::BehaviorSubject, prelude::*};
use std::sync::{Arc, Mutex};

//todo use atomic bool replace Box<dyn Publisher<Item = Item, Err = Err> + Send
// + Sync>
pub struct SharedSubject<Item, Err>(
  Arc<
    Mutex<
      Subject<
        Box<dyn Observer<Item = Item, Err = Err> + Send + Sync>,
        Arc<Mutex<SingleSubscription>>,
      >,
    >,
  >,
);

pub struct SharedBehaviorSubject<Item, Err>(
  Arc<
    Mutex<
      BehaviorSubject<
        Box<dyn Observer<Item = Item, Err = Err> + Send + Sync>,
        Arc<Mutex<SingleSubscription>>,
      >,
    >,
  >,
);

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
    self.0.lock().unwrap().subscribed_size()
  }
}

impl<Item, Err> Observable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Unsub = Arc<Mutex<SingleSubscription>>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self.0.lock().unwrap().subscribe(Box::new(observer))
  }
}

impl<Item, Err> Observable for SharedBehaviorSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedBehaviorSubject<Item, Err>
where
  Item: Clone,
  Err: Clone,
{
  type Unsub = Arc<Mutex<SingleSubscription>>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let mut inner = self.0.lock().unwrap();
    if !inner.subject.is_closed() {
      let v = inner.value.clone();
      inner.subject.next(v);
    }
    inner.subject.subscribe(Box::new(observer))
  }
}

impl<Item, Err> SharedBehaviorSubject<Item, Err> {
  #[inline]
  pub fn new(value: Item) -> Self {
    SharedBehaviorSubject(Arc::new(Mutex::new(BehaviorSubject {
      subject: Default::default(),
      value,
    })))
  }
}

impl<Item, Err> Default for SharedSubject<Item, Err> {
  fn default() -> Self { Self(Default::default()) }
}

impl<Item, Err> Clone for SharedSubject<Item, Err> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<Item, Err> Clone for SharedBehaviorSubject<Item, Err> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<Item, Err> Observer for SharedSubject<Item, Err>
where
  Item: Clone,
  Err: Clone,
{
  type Item = Item;

  type Err = Err;

  fn next(&mut self, value: Self::Item) { self.0.lock().unwrap().next(value) }

  fn error(&mut self, err: Self::Err) { self.0.lock().unwrap().error(err) }

  fn complete(&mut self) { self.0.lock().unwrap().complete() }
}

impl<Item, Err> Observer for SharedBehaviorSubject<Item, Err>
where
  Item: Clone,
  Err: Clone,
{
  type Item = Item;

  type Err = Err;

  fn next(&mut self, value: Self::Item) { self.0.lock().unwrap().next(value) }

  fn error(&mut self, err: Self::Err) { self.0.lock().unwrap().error(err) }

  fn complete(&mut self) { self.0.lock().unwrap().complete() }
}

impl<Item, Err> SubscriptionLike for SharedSubject<Item, Err> {
  fn unsubscribe(&mut self) { self.0.lock().unwrap().unsubscribe() }

  fn is_closed(&self) -> bool { self.0.lock().unwrap().is_closed() }
}
impl<Item, Err> TearDownSize for SharedSubject<Item, Err> {
  fn teardown_size(&self) -> usize { self.0.lock().unwrap().observers.len() }
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
