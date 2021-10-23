use crate::prelude::*;
use std::fmt::{Debug, Formatter};
mod local_subject;
pub use local_subject::*;

mod shared_subject;
pub use shared_subject::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Subject<V, S> {
  pub(crate) observers: SubjectObserver<V>,
  pub(crate) subscription: S,
}

impl<O, U: SubscriptionLike> SubscriptionLike for Subject<O, U> {
  #[inline]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

macro_rules! impl_observer {
  () => {
    #[inline]
    fn next(&mut self, value: Item) { self.observers.next(value) }

    #[inline]
    fn error(&mut self, err: Err) { self.observers.error(err) }

    #[inline]
    fn complete(&mut self) { self.observers.complete() }
  };
}

impl<Item, Err, U, O> Observer for Subject<Arc<Mutex<Vec<O>>>, U>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;

  impl_observer!();
}

impl<Item, Err, U, O> Observer for Subject<Rc<RefCell<Vec<O>>>, U>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;

  impl_observer!();
}

impl<Item, Err, U, O> Observer for Subject<Box<Vec<O>>, U>
where
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  impl_observer!();
}

#[derive(Default, Clone)]
pub(crate) struct SubjectObserver<V> {
  pub(crate) observers: V,
}

impl<Item, Err, O> Observer for SubjectObserver<Arc<Mutex<Vec<O>>>>
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

impl<Item, Err, O> Observer for SubjectObserver<Rc<RefCell<Vec<O>>>>
where
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    {
      let any_finished = self.observers.borrow_mut().iter_mut().fold(
        false,
        |finished, o: &mut _| {
          o.next(value.clone());
          finished || o.is_finished()
        },
      );
      if any_finished {
        self.observers.borrow_mut().retain(|o| !o.is_finished());
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

impl<Item, Err, O> Observer for SubjectObserver<Box<Vec<O>>>
where
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    let any_finished =
      self
        .observers
        .iter_mut()
        .fold(false, |finished, o: &mut _| {
          o.next(value.clone());
          finished || o.is_finished()
        });
    if any_finished {
      self.observers.retain(|o| !o.is_finished());
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
impl<O, S> Debug for Subject<Arc<Mutex<Vec<O>>>, S> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field(
        "observer_count",
        &self.observers.observers.lock().unwrap().len(),
      )
      .finish()
  }
}
impl<O, S> Debug for Subject<Rc<RefCell<Vec<O>>>, S> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field(
        "observer_count",
        &self.observers.observers.borrow_mut().len(),
      )
      .finish()
  }
}

impl<O, S> Debug for Subject<Box<Vec<O>>, S> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field("observer_count", &self.observers.observers.len())
      .finish()
  }
}
#[cfg(test)]
mod test {
  use super::*;
  use futures::executor::ThreadPool;
  use std::time::{Duration, Instant};

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = LocalSubject::new();
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = LocalSubject::new();
    broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!("{}", e));
    broadcast.next(1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = LocalSubject::new();
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn empty_local_subject_can_convert_into_shared() {
    let pool = ThreadPool::new().unwrap();
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let subject = SharedSubject::new();
    let mut subject_c = subject.clone();
    let stamp = Instant::now();
    pool.schedule(
      move |_| {
        subject_c.next(100);
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
    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = LocalSubject::new();
    let local2 = LocalSubject::new();
    local.clone().actual_subscribe(Subscriber {
      observer: local2.observers,
      subscription: local2.subscription,
    });
    local.next(1);
    local.error(2);
  }

  #[test]
  fn fix_recursive_next() {
    let mut subject = LocalSubject::new();
    let mut c_subject = subject.clone();

    subject.clone().subscribe(move |i| {
      if i < 1 {
        c_subject.next(2)
      }
    });

    subject.next(0);

    let mut subject = SharedSubject::new();
    let mut c_subject = subject.clone();

    subject.clone().into_shared().subscribe(move |i| {
      if i < 1 {
        c_subject.next(2)
      }
    });

    subject.next(0);
  }
}
