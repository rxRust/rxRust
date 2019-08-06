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
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
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
  fn next(&self, v: &Item) -> RxReturn<Err> {
    if self.stopped.load(Ordering::Relaxed) {
      return RxReturn::Continue;
    };
    let mut publishers = self.cbs.lock().unwrap();
    publishers.drain_filter(|subscriber| {
      let mut subscriber = subscriber.lock().unwrap();
      match subscriber.next(&v) {
        RxReturn::Complete => {
          subscriber.complete();
        }
        RxReturn::Err(err) => {
          subscriber.error(&err);
        }
        _ => {}
      };
      subscriber.is_stopped()
    });

    RxReturn::Continue
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
      RxReturn::Continue
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

  use crate::{prelude::*, subscribable::Subscribable};
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
  #[should_panic]
  fn runtime_error() {
    let subject = Subject::new();
    subject.clone().raw_subscribe(RxFnWrapper::new(
      |v: RxValue<&'_ _, &'_ _>| match v {
        RxValue::Next(_) => RxReturn::Err("runtime error"),
        RxValue::Err(e) => panic!(*e),
        _ => RxReturn::Continue,
      },
    ));

    subject.next(&1);
  }

  #[test]
  fn return_err_state() {
    let ec = Arc::new(Mutex::new(0));
    let c_ec = ec.clone();
    let subject = Subject::new();
    subject.clone().raw_subscribe(RxFnWrapper::new(
      move |v: RxValue<&'_ _, &'_ _>| match v {
        RxValue::Next(_) => RxReturn::Err("runtime error"),
        RxValue::Err(_) => {
          *ec.lock().unwrap() += 1;
          RxReturn::Continue
        }
        _ => RxReturn::Continue,
      },
    ));

    subject.next(&1);
    assert_eq!(*c_ec.lock().unwrap(), 1);
    // should stopped
    subject.next(&1);
    assert_eq!(*c_ec.lock().unwrap(), 1);
  }

  #[test]
  fn return_complete_state() {
    let cc = Arc::new(Mutex::new(0));
    let c_cc = cc.clone();
    let broadcast = Subject::new();
    broadcast.clone().raw_subscribe(RxFnWrapper::new(
      move |v: RxValue<&'_ i32, &'_ ()>| match v {
        RxValue::Next(_) => RxReturn::Complete,
        RxValue::Err(_) => RxReturn::Continue,
        _ => {
          *cc.lock().unwrap() += 1;
          RxReturn::Continue
        }
      },
    ));

    broadcast.next(&1);
    assert_eq!(*c_cc.lock().unwrap(), 1);
    // should stopped
    broadcast.next(&1);
    assert_eq!(*c_cc.lock().unwrap(), 1);
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
