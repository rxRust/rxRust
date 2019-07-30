use crate::prelude::*;
use std::marker::PhantomData;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc, Mutex,
};

#[derive(Default)]
pub struct Subject<Item, Err> {
  cbs: Arc<Mutex<Vec<RefPublisher<Item, Err>>>>,
  stopped: Arc<AtomicBool>,
  _p: PhantomData<(Item, Err)>,
}

type RefPublisher<Item, Err> = Arc<Mutex<Box<dyn Publisher<Item = Item, Err = Err> + Send + Sync>>>;

impl<'a, T, E> Clone for Subject<T, E> {
  fn clone(&self) -> Self {
    Subject {
      cbs: self.cbs.clone(),
      stopped: self.stopped.clone(),
      _p: PhantomData,
    }
  }
}

impl<Item: 'static, Err: 'static> ImplSubscribable for Subject<Item, Err> {
  type Item = Item;
  type Err = Err;

  fn subscribe_return_state(
    self,
    subscribe: impl RxFn(RxValue<'_, Self::Item, Self::Err>) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let subscriber = Subscriber::new(subscribe);

    let subscriber: Box<dyn Publisher<Item = Item, Err = Err> + Send + Sync> = Box::new(subscriber);
    let subscriber = Arc::new(Mutex::new(subscriber));
    self.cbs.lock().unwrap().push(subscriber.clone());

    Box::new(subscriber)
  }
}

impl<Item: 'static, Err: 'static> Fork for Subject<Item, Err> {
  type Output = Self;
  fn fork(&self) -> Self::Output { self.clone() }
}

impl<Item: 'static, Err: 'static> Multicast for Subject<Item, Err> {
  type Output = Self;
  #[inline(always)]
  fn multicast(self) -> Self::Output { self }
}

impl<'a, Item: 'a, Err: 'a> Subject<Item, Err> {
  pub fn new() -> Self {
    Subject {
      cbs: Arc::new(Mutex::new(vec![])),
      stopped: Arc::new(AtomicBool::new(false)),
      _p: PhantomData,
    }
  }
}

impl<Item, Err> Subscription for RefPublisher<Item, Err> {
  #[inline]
  fn unsubscribe(&mut self) { self.lock().unwrap().unsubscribe(); }
}

// completed return or unsubscribe called.
impl<T, E> Observer for Subject<T, E> {
  type Item = T;
  type Err = E;

  fn next(&self, v: &Self::Item) -> RxReturn<Self::Err> {
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

  fn complete(&mut self) {
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

  fn error(&mut self, err: &Self::Err) {
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

  fn is_stopped(&self) -> bool { self.stopped.load(Ordering::Relaxed) }
}

#[test]
fn base_data_flow() {
  use std::sync::{Arc, Mutex};
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
  let mut broadcast = Subject::new();
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
  subject
    .clone()
    .subscribe_return_state(RxFnWrapper::new(|v: RxValue<'_, _, _>| match v {
      RxValue::Next(_) => RxReturn::Err("runtime error"),
      RxValue::Err(e) => panic!(*e),
      _ => RxReturn::Continue,
    }));

  subject.next(&1);
}

#[test]
fn return_err_state() {
  use std::sync::{Arc, Mutex};
  let ec = Arc::new(Mutex::new(0));
  let c_ec = ec.clone();
  let subject = Subject::new();
  subject
    .clone()
    .subscribe_return_state(RxFnWrapper::new(move |v: RxValue<'_, _, _>| match v {
      RxValue::Next(_) => RxReturn::Err("runtime error"),
      RxValue::Err(_) => {
        *ec.lock().unwrap() += 1;
        RxReturn::Continue
      }
      _ => RxReturn::Continue,
    }));

  subject.next(&1);
  assert_eq!(*c_ec.lock().unwrap(), 1);
  // should stopped
  subject.next(&1);
  assert_eq!(*c_ec.lock().unwrap(), 1);
}

#[test]
fn return_complete_state() {
  use std::sync::{Arc, Mutex};
  let cc = Arc::new(Mutex::new(0));
  let c_cc = cc.clone();
  let broadcast = Subject::new();
  broadcast.clone().subscribe_return_state(RxFnWrapper::new(
    move |v: RxValue<'_, i32, ()>| match v {
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
  use std::sync::{Arc, Mutex};
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
