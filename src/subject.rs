use crate::prelude::*;
mod local_subject;
pub use local_subject::*;

mod shared_subject;
pub use shared_subject::*;

#[derive(Default, Clone)]
pub struct Subject<O, S> {
  pub(crate) observers: O,
  pub(crate) subscription: S,
}

impl<O, U: SubscriptionLike> SubscriptionLike for Subject<O, U> {
  #[inline]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<Item, Err, U, O> Observer for Subject<O, U>
where
  O: Observer<Item = Item, Err = Err>,
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
}

#[derive(Clone)]
pub struct SubjectObserver<O> {
  pub(crate) observers: Vec<O>,
}

impl<O: Publisher> Observer for SubjectObserver<O>
where
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;
  fn next(&mut self, value: Self::Item) {
    let any_finished = self.observers.iter_mut().fold(false, |finished, o| {
      o.next(value.clone());
      finished || o.is_finished()
    });
    if any_finished {
      self.observers.retain(|o| !o.is_finished());
    }
  }

  fn error(&mut self, err: Self::Err) {
    self
      .observers
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    self.observers.clear();
  }

  fn complete(&mut self) {
    self
      .observers
      .iter_mut()
      .for_each(|subscriber| subscriber.complete());
    self.observers.clear();
  }
}

impl<O> Default for SubjectObserver<O> {
  fn default() -> Self { SubjectObserver { observers: vec![] } }
}

impl<O> std::ops::Deref for SubjectObserver<O> {
  type Target = Vec<O>;
  fn deref(&self) -> &Self::Target { &self.observers }
}

impl<O> std::ops::DerefMut for SubjectObserver<O> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.observers }
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

    subject.observers.next(0);

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
