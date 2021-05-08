use crate::prelude::*;
use std::fmt::{Debug, Formatter};
mod local_subject;
pub use local_subject::*;

mod shared_subject;
pub use shared_subject::*;

#[derive(Default, Clone)]
pub struct Subject<V, S> {
  pub(crate) observers: SubjectObserver<V>,
  pub(crate) subscription: S,
}

impl<V, S, O> Subject<V, S>
where
  V: InnerDeref<Target = Vec<O>>,
{
  #[inline]
  pub fn new() -> Self
  where
    Self: Default,
  {
    Self::default()
  }
  #[inline]
  pub fn subscribed_size(&self) -> usize {
    self.observers.observers.inner_deref().len()
  }
}

impl<O, U: SubscriptionLike> SubscriptionLike for Subject<O, U> {
  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }

  #[inline]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }
}

impl<Item, Err, U, O, V> Observer for Subject<V, U>
where
  V: InnerDerefMut<Target = Vec<O>>,
  O: Observer<Item = Item, Err = Err> + SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  #[inline]
  fn next(&mut self, value: Item) { self.observers.next(value) }

  #[inline]
  fn error(&mut self, err: Err) { self.observers.error(err) }

  #[inline]
  fn complete(&mut self) { self.observers.complete() }

  #[inline]
  fn is_stopped(&self) -> bool { self.observers.is_stopped() }
}

#[derive(Default, Clone)]
pub(crate) struct SubjectObserver<V> {
  pub(crate) observers: V,
  is_stopped: bool,
}

impl<Item, Err, V, O> Observer for SubjectObserver<V>
where
  V: InnerDerefMut<Target = Vec<O>>,
  O: Publisher<Item = Item, Err = Err>,
  Item: Clone,
  Err: Clone,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    {
      let mut vec = self.observers.inner_deref_mut();
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
    let mut observers = self.observers.inner_deref_mut();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    observers.clear();
    self.is_stopped = true;
  }

  fn complete(&mut self) {
    let mut observers = self.observers.inner_deref_mut();
    observers
      .iter_mut()
      .for_each(|subscriber| subscriber.complete());
    observers.clear();
    self.is_stopped = true;
  }

  #[inline]
  fn is_stopped(&self) -> bool { self.is_stopped }
}

impl<V, O, S> Debug for Subject<V, S>
where
  V: InnerDeref<Target = Vec<O>>,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalSubject")
      .field(
        "observer_count",
        &self.observers.observers.inner_deref().len(),
      )
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
      let mut broadcast = Subject::new();
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::new();
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
      let mut subject = Subject::new();
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
    let subject = Subject::new();
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
}
