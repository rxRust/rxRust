use crate::observer::Observer;
use crate::prelude::{Subject, SubscriptionLike};

#[derive(Default, Clone)]
pub struct BehaviorSubject<O, U, V> {
  pub(crate) subject: Subject<O, U>,
  pub(crate) value: V,
}

impl<O, U: SubscriptionLike, V> SubscriptionLike for BehaviorSubject<O, U, V> {
  #[inline]
  fn unsubscribe(&mut self) { self.subject.unsubscribe(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subject.is_closed() }
}

impl<O, U> Observer for BehaviorSubject<O, U, O::Item>
where
  O: Observer,
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;

  #[inline]
  fn next(&mut self, value: Self::Item) {
    self.value = value;
    self.subject.next(self.value.clone())
  }

  #[inline]
  fn error(&mut self, err: Self::Err) { self.subject.error(err) }

  #[inline]
  fn complete(&mut self) { self.subject.complete() }
}

#[cfg(test)]
mod test {
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
      observer: local2.subject.observers,
      subscription: local2.subject.subscription,
    });
    local.next(1);
    local.error(2);
  }
}
