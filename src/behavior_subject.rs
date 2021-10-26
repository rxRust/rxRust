use crate::observer::Observer;
use crate::prelude::{Publisher, SubscriptionLike};
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct BehaviorSubject<V, S, I> {
  pub(crate) observers: BehaviorSubjectObserver<V>,
  pub(crate) subscription: S,
  pub(crate) value: I,
}

impl<O, U: SubscriptionLike, V> SubscriptionLike for BehaviorSubject<O, U, V> {
  #[inline]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

macro_rules! impl_behavior_observer {
  () => {
    #[inline]
    fn next(&mut self, value: Item) {
      self.value = value;
      self.observers.next(self.value.clone())
    }

    #[inline]
    fn error(&mut self, err: Err) { self.observers.error(err) }

    #[inline]
    fn complete(&mut self) { self.observers.complete() }
  };
}

impl<Item, Err, U, O> Observer for BehaviorSubject<Arc<Mutex<Vec<O>>>, U, Item>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;

  impl_behavior_observer!();
}

impl<Item, Err, U, O> Observer for BehaviorSubject<Rc<RefCell<Vec<O>>>, U, Item>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;

  impl_behavior_observer!();
}

impl<Item, Err, U, O> Observer for BehaviorSubject<Box<Vec<O>>, U, Item>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  impl_behavior_observer!();
}

#[derive(Default, Clone)]
pub(crate) struct BehaviorSubjectObserver<V> {
  pub(crate) observers: V,
}

impl<Item, Err, O> Observer for BehaviorSubjectObserver<Arc<Mutex<Vec<O>>>>
where
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    {
      let mut vec = self.observers.lock().unwrap();
      let not_done: Vec<O> = vec
        .drain(..)
        .map(|mut o| {
          o.next(value.clone());
          o
        })
        .filter(|o| !o.is_finished())
        .collect();
      for p in not_done {
        vec.push(p);
      }
    }
  }

  fn error(&mut self, err: Err) {
    let mut observers = self.observers.lock().unwrap();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    observers.clear();
  }

  fn complete(&mut self) {
    let mut observers = self.observers.lock().unwrap();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.complete());
    observers.clear();
  }
}

impl<Item, Err, O> Observer for BehaviorSubjectObserver<Rc<RefCell<Vec<O>>>>
where
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    {
      let mut vec = self.observers.borrow_mut();
      let not_done: Vec<O> = vec
        .drain(..)
        .map(|mut o| {
          o.next(value.clone());
          o
        })
        .filter(|o| !o.is_finished())
        .collect();
      for p in not_done {
        vec.push(p);
      }
    }
  }

  fn error(&mut self, err: Err) {
    let mut observers = self.observers.borrow_mut();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    observers.clear();
  }

  fn complete(&mut self) {
    let mut observers = self.observers.borrow_mut();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.complete());
    observers.clear();
  }
}

impl<Item, Err, O> Observer for BehaviorSubjectObserver<Box<Vec<O>>>
where
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    {
      let vec = &mut self.observers;
      let not_done: Vec<O> = vec
        .drain(..)
        .map(|mut o| {
          o.next(value.clone());
          o
        })
        .filter(|o| !o.is_finished())
        .collect();
      for p in not_done {
        vec.push(p);
      }
    }
  }

  fn error(&mut self, err: Err) {
    let observers = &mut self.observers;
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    observers.clear();
  }

  fn complete(&mut self) {
    let observers = &mut self.observers;
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.complete());
    observers.clear();
  }
}
impl<O, S, V> Debug for BehaviorSubject<Arc<Mutex<Vec<O>>>, S, V> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field(
        "observer_count",
        &self.observers.observers.lock().unwrap().len(),
      )
      .finish()
  }
}
impl<O, S, V> Debug for BehaviorSubject<Rc<RefCell<Vec<O>>>, S, V> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field(
        "observer_count",
        &self.observers.observers.borrow_mut().len(),
      )
      .finish()
  }
}

impl<O, S, V> Debug for BehaviorSubject<Box<Vec<O>>, S, V> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field("observer_count", &self.observers.observers.len())
      .finish()
  }
}
#[cfg(test)]
mod test {
  use super::*;
  use crate::prelude::*;
  use futures::executor::ThreadPool;
  use std::time::{Duration, Instant};

  #[test]
  fn base_data_flow() {
    let mut i = 0;

    {
      let broadcast = LocalBehaviorSubject::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
    }

    assert_eq!(i, 84);

    {
      let mut broadcast = LocalBehaviorSubject::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }

    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = LocalBehaviorSubject::new(42);
    broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!("{}", e));

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let subject = LocalBehaviorSubject::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
    }

    assert_eq!(i, 42);

    {
      let mut subject = LocalBehaviorSubject::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 42);
  }

  #[test]
  fn empty_local_subject_can_convert_into_shared() {
    let pool = ThreadPool::new().unwrap();
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let subject = SharedBehaviorSubject::new(42);
    let mut subject_c = subject.clone();
    let stamp = Instant::now();
    pool.schedule(
      move |_| {
        subject_c.complete();
      },
      Some(Duration::from_millis(25)),
      (),
    );
    subject
      .clone()
      .into_shared()
      .observe_on(pool)
      .into_shared()
      .subscribe_blocking(move |v: i32| {
        *value.lock().unwrap() = v;
      });
    assert!(stamp.elapsed() > Duration::from_millis(25));
    assert_eq!(*c_v.lock().unwrap(), 42);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = LocalBehaviorSubject::new(42);
    let local2 = LocalBehaviorSubject::new(42);
    local.clone().actual_subscribe(Subscriber {
      observer: local2.observers,
      subscription: local2.subscription,
    });
    local.next(1);
    local.error(2);
  }
}
