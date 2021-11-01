use crate::prelude::*;
mod local_subject;
pub use local_subject::*;

mod shared_subject;
pub use shared_subject::*;

#[derive(Clone)]
pub struct Subject<O: Observer, S: SubscriptionLike> {
  pub(crate) observers: Vec<SubjectObserver<O, S>>,
  pub(crate) subscription: SingleSubscription,
}

impl<O: Observer, U: SubscriptionLike> SubscriptionLike for Subject<O, U> {
  #[inline]
  fn unsubscribe(&mut self) {
    self
      .observers
      .iter_mut()
      .for_each(|o| o.subscription.unsubscribe());
    self.observers.clear();
    self.subscription.unsubscribe();
  }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<O: Observer, U: SubscriptionLike> Subject<O, U> {
  fn subscribed_size(&self) -> usize { self.observers.len() }
}

#[derive(Clone)]
pub(crate) struct SubjectObserver<O: Observer, S: SubscriptionLike> {
  pub observer: O,
  pub subscription: S,
}

impl<O: Observer, S: SubscriptionLike> Observer for Subject<O, S>
where
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;
  fn next(&mut self, value: Self::Item) {
    let any_finished = self.observers.iter_mut().fold(false, |finished, o| {
      o.observer.next(value.clone());
      finished || o.subscription.is_closed()
    });
    if any_finished {
      self.observers.retain(|o| !o.subscription.is_closed());
    }
  }

  fn error(&mut self, err: Self::Err) {
    self
      .observers
      .iter_mut()
      .for_each(|subscriber| subscriber.observer.error(err.clone()));
    self.observers.clear();
  }

  fn complete(&mut self) {
    self
      .observers
      .iter_mut()
      .for_each(|subscriber| subscriber.observer.complete());
    self.observers.clear();
  }
}

impl<O: Observer, S: SubscriptionLike> Default for Subject<O, S> {
  fn default() -> Self {
    Subject {
      observers: vec![],
      subscription: Default::default(),
    }
  }
}

impl<O, U> Subject<O, U>
where
  O: Observer,
  U: SubscriptionLike + Default + Clone,
{
  fn subscribe(&mut self, observer: O) -> U {
    let subscription = U::default();
    let observer = SubjectObserver {
      observer,
      subscription: subscription.clone(),
    };
    self.observers.push(observer);
    subscription
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
      let mut broadcast = LocalSubject::default();
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
    local.clone().actual_subscribe(local2);
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
