use crate::prelude::*;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc, Mutex,
};

#[derive(Default)]
pub struct Subject<Item, Err> {
  cbs: Arc<Mutex<Vec<RefPublisher<Item, Err>>>>,
  stopped: Arc<AtomicBool>,
}

type RefPublisher<Item, Err> =
  Arc<Mutex<Box<dyn Observer<Item, Err> + Send + Sync>>>;

impl<'a, T, E> Clone for Subject<T, E> {
  fn clone(&self) -> Self {
    Subject {
      cbs: self.cbs.clone(),
      stopped: self.stopped.clone(),
    }
  }
}

impl<Item, Err> RawSubscribable for Subject<Item, Err> {
  type Item = Item;
  type Err = Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let subscriber = Subscriber::new(subscribe);
    let subscription = subscriber.clone_subscription();

    let subscriber: Box<dyn Observer<Item, Err> + Send + Sync> =
      Box::new(subscriber);
    let subscriber = Arc::new(Mutex::new(subscriber));
    self.cbs.lock().unwrap().push(subscriber.clone());

    Box::new(subscription)
  }
}

impl<Item, Err> Fork for Subject<Item, Err> {
  type Output = Self;
  fn fork(&self) -> Self::Output { self.clone() }
}

impl<Item, Err> Multicast for Subject<Item, Err> {
  type Output = Self;
  #[inline(always)]
  fn multicast(self) -> Self::Output { self }
}

impl<'a, Item: 'a, Err: 'a> Subject<Item, Err> {
  pub fn new() -> Self {
    Subject {
      cbs: Arc::new(Mutex::new(vec![])),
      stopped: Arc::new(AtomicBool::new(false)),
    }
  }
}

// completed return or unsubscribe called.
impl<Item, Err> Observer<Item, Err> for Subject<Item, Err> {
  fn next(&self, v: &Item) {
    if self.stopped.load(Ordering::Relaxed) {};
    let mut publishers = self.cbs.lock().unwrap();
    publishers.drain_filter(|subscriber| {
      let subscriber = subscriber.lock().unwrap();
      subscriber.next(&v);
      subscriber.is_stopped()
    });
  }

  #[inline(always)]
  fn complete(&mut self) { Subject::complete(self) }

  #[inline(always)]
  fn error(&mut self, err: &Err) { Subject::error(self, err) }

  fn is_stopped(&self) -> bool { self.stopped.load(Ordering::Relaxed) }
}

impl<Item, Err> Subject<Item, Err> {
  pub fn from_subscribable(
    o: impl RawSubscribable<Item = Item, Err = Err>,
  ) -> Self
  where
    Item: 'static,
    Err: 'static,
  {
    let subject = Subject::new();
    let r_subject = subject.fork();
    o.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
      match v {
        RxValue::Next(value) => {
          subject.next(value);
        }
        RxValue::Err(err) => subject.error(err),
        RxValue::Complete => subject.complete(),
      };
    }));
    r_subject
  }

  fn complete(&self) {
    if self.stopped.load(Ordering::Relaxed) {
      return;
    };
    let mut publishers = self.cbs.lock().unwrap();
    publishers.iter().for_each(|subscriber| {
      subscriber.lock().unwrap().complete();
    });
    publishers.clear();

    self.stopped.store(true, Ordering::Relaxed);
  }

  fn error(&self, err: &Err) {
    if self.stopped.load(Ordering::Relaxed) {
      return;
    };
    let mut publishers = self.cbs.lock().unwrap();
    publishers.iter().for_each(|subscriber| {
      subscriber.lock().unwrap().error(err);
    });
    publishers.clear();
    self.stopped.store(true, Ordering::Relaxed);
  }
}

#[cfg(test)]
mod test {

  use crate::prelude::*;
  use std::sync::{Arc, Mutex};

  #[test]
  fn base_data_flow() {
    let i = Arc::new(Mutex::new(0));
    let c_i = i.clone();
    let broadcast = Subject::<i32, ()>::new();
    broadcast
      .clone()
      .subscribe(move |v: &i32| *i.lock().unwrap() = *v * 2);
    broadcast.next(&1);
    assert_eq!(*c_i.lock().unwrap(), 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let broadcast = Subject::new();
    broadcast
      .clone()
      .subscribe_err(|_: &i32| {}, |e: &_| panic!(*e));
    broadcast.next(&1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let i = Arc::new(Mutex::new(0));
    let c_i = i.clone();
    let subject = Subject::<_, ()>::new();
    subject
      .clone()
      .subscribe(move |v| *i.lock().unwrap() = *v)
      .unsubscribe();
    subject.next(&100);
    assert_eq!(*c_i.lock().unwrap(), 0);
  }

  #[test]
  fn fork() {
    let subject = Subject::<(), ()>::new();
    subject.fork().fork().fork().fork();
  }

  #[test]
  #[should_panic]
  fn from_subscribable() {
    let o = Subject::new();
    Subject::<(), ()>::from_subscribable(o.clone())
      .subscribe(|_| panic!("hit next"));
    o.next(&());
  }
}
