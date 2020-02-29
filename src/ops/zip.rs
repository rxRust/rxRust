use crate::prelude::*;
use observer::{complete_proxy_impl, error_proxy_impl};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// An Observable that combines from two other two Observables.
///
/// This struct is created by the zip method on [Observable](Observable::zip).
/// See its documentation for more.
#[derive(Clone)]
pub struct ZipOp<A, B> {
  pub(crate) a: A,
  pub(crate) b: B,
}

impl<A, B> Observable for ZipOp<A, B>
where
  A: Observable,
  B: Observable<Err = A::Err>,
{
  type Item = (A::Item, B::Item);
  type Err = A::Err;
}

impl<'a, A, B> LocalObservable<'a> for ZipOp<A, B>
where
  A: LocalObservable<'a>,
  B: LocalObservable<'a, Err = A::Err>,
  A::Item: 'a,
  B::Item: 'a,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let mut sub = subscriber.subscription;
    let o_zip = ZipObserver::new(subscriber.observer, sub.clone());
    let o_zip = Rc::new(RefCell::new(o_zip));
    sub.add(self.a.actual_subscribe(Subscriber {
      observer: AObserver(o_zip.clone(), PhantomData),
      subscription: LocalSubscription::default(),
    }));

    sub.add(self.b.actual_subscribe(Subscriber {
      observer: BObserver(o_zip, PhantomData),
      subscription: LocalSubscription::default(),
    }));
    sub
  }
}

impl<A, B> SharedObservable for ZipOp<A, B>
where
  A: SharedObservable,
  B: SharedObservable<Err = A::Err>,
  A::Item: Send + Sync + 'static,
  B::Item: Send + Sync + 'static,
  A::Unsub: Send + Sync,
  B::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let mut sub = subscriber.subscription;
    let o_zip = ZipObserver::new(subscriber.observer, sub.clone());
    let o_zip = Arc::new(Mutex::new(o_zip));
    sub.add(self.a.actual_subscribe(Subscriber {
      observer: AObserver(o_zip.clone(), PhantomData),
      subscription: SharedSubscription::default(),
    }));

    sub.add(self.b.actual_subscribe(Subscriber {
      observer: BObserver(o_zip, PhantomData),
      subscription: SharedSubscription::default(),
    }));
    sub
  }
}

enum ZipItem<A, B> {
  ItemA(A),
  ItemB(B),
}

struct ZipObserver<O, U, A, B> {
  observer: O,
  subscription: U,
  a: VecDeque<A>,
  b: VecDeque<B>,
  completed_one: bool,
}

impl<O, U, A, B> ZipObserver<O, U, A, B> {
  fn new(o: O, u: U) -> Self {
    ZipObserver {
      observer: o,
      subscription: u,
      a: VecDeque::default(),
      b: VecDeque::default(),
      completed_one: false,
    }
  }
}

impl<O, U, A, B, Err> Observer<ZipItem<A, B>, Err> for ZipObserver<O, U, A, B>
where
  O: Observer<(A, B), Err>,
  U: SubscriptionLike,
{
  fn next(&mut self, value: ZipItem<A, B>) {
    match value {
      ZipItem::ItemA(v) => {
        if !self.b.is_empty() {
          self.observer.next((v, self.b.pop_front().unwrap()))
        } else {
          self.a.push_back(v);
        }
      }
      ZipItem::ItemB(v) => {
        if !self.a.is_empty() {
          self.observer.next((self.a.pop_front().unwrap(), v))
        } else {
          self.b.push_back(v)
        }
      }
    }
  }

  fn error(&mut self, err: Err) {
    self.observer.error(err);
    self.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    if self.completed_one {
      self.subscription.unsubscribe();
      self.observer.complete();
    } else {
      self.completed_one = true;
    }
  }
}

struct AObserver<O, B>(O, PhantomData<B>);

impl<O, A, B, Err> Observer<A, Err> for AObserver<O, B>
where
  O: Observer<ZipItem<A, B>, Err>,
{
  fn next(&mut self, value: A) { self.0.next(ZipItem::ItemA(value)); }

  error_proxy_impl!(Err, 0);
  complete_proxy_impl!(0);
}

struct BObserver<O, A>(O, PhantomData<A>);

impl<O, A, B, Err> Observer<B, Err> for BObserver<O, A>
where
  O: Observer<ZipItem<A, B>, Err>,
{
  fn next(&mut self, value: B) { self.0.next(ZipItem::ItemB(value)); }

  error_proxy_impl!(Err, 0);
  complete_proxy_impl!(0);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[test]
  fn smoke() {
    let zip = observable::from_iter(0..10).zip(observable::from_iter(0..10));
    let zipped_count = Arc::new(AtomicUsize::new(0));
    let zcc = zipped_count.clone();
    zip
      .clone()
      .count()
      .subscribe(|v| zipped_count.store(v, Ordering::Relaxed));
    let mut zipped_sum = 0;
    assert_eq!(zcc.load(Ordering::Relaxed), 10);
    zip.map(|(a, b)| a + b).sum().subscribe(|v| zipped_sum = v);
    assert_eq!(zipped_sum, 90);
  }

  #[test]
  fn complete() {
    let mut complete = false;
    {
      let mut s1 = Subject::new();
      s1.clone()
        .zip(Subject::new())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
    }
    assert_eq!(complete, false);

    {
      let mut s1 = Subject::new();
      let mut s2 = Subject::new();
      s1.clone()
        .zip(s2.clone())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
      s2.complete();
    }
    assert_eq!(complete, true);
  }
}
