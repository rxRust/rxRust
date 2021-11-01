/// Make a ConnectableObservable behave like a ordinary observable and
/// automates the way you can connect to it.
///
/// Internally it counts the subscriptions to the observable and subscribes
/// (only once) to the source if the number of subscriptions is larger than
/// 0. If the number of subscriptions is smaller than 1, it unsubscribes
/// from the source. This way you can make sure that everything before the
/// published refCount has only a single subscription independently of the
/// number of subscribers to the target observable.
///
/// Note that using the share operator is exactly the same as using the
/// publish operator (making the observable hot) and the refCount operator
/// in a sequence.
use crate::observable::{
  LocalConnectableObservable, SharedConnectableObservable,
};
use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct Inner<C, U> {
  connectable: C,
  connection: Option<U>,
}

pub struct RefCount<T, C>(T, TypeHint<C>);

impl<T: Clone, C> Clone for RefCount<T, C> {
  fn clone(&self) -> Self { RefCount(self.0.clone(), TypeHint::new()) }
}

type LocalRef<C, U> = Rc<RefCell<Inner<C, U>>>;

pub struct InnerLocalRefCount<'a, S, Item, Err>(
  LocalRef<LocalConnectableObservable<'a, S, Item, Err>, S::Unsub>,
)
where
  S: LocalObservable<'a, Item = Item, Err = Err>;

pub type LocalRefCount<'a, S, Item, Err> = RefCount<
  InnerLocalRefCount<'a, S, Item, Err>,
  LocalConnectableObservable<'a, S, Item, Err>,
>;

impl<'a, S, Item, Err> Clone for InnerLocalRefCount<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
{
  #[inline]
  fn clone(&self) -> Self { InnerLocalRefCount(self.0.clone()) }
}

type SharedRef<C, U> = Arc<Mutex<Inner<C, U>>>;

pub struct InnerSharedRefCount<S, Item, Err>(
  SharedRef<SharedConnectableObservable<S, Item, Err>, S::Unsub>,
)
where
  S: SharedObservable<Item = Item, Err = Err>;

impl<S, Item, Err> Clone for InnerSharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
{
  #[inline]
  fn clone(&self) -> Self { InnerSharedRefCount(self.0.clone()) }
}

pub type SharedRefCount<S, Item, Err> = RefCount<
  InnerSharedRefCount<S, Item, Err>,
  SharedConnectableObservable<S, Item, Err>,
>;

pub trait RefCountCreator: Sized {
  type Connectable;
  fn new(connectable: Self::Connectable) -> RefCount<Self, Self::Connectable>;
}

impl<'a, S, Item, Err> RefCountCreator for InnerLocalRefCount<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
{
  type Connectable = LocalConnectableObservable<'a, S, Item, Err>;
  fn new(connectable: Self::Connectable) -> RefCount<Self, Self::Connectable> {
    RefCount(
      InnerLocalRefCount(Rc::new(RefCell::new(Inner {
        connectable,
        connection: None,
      }))),
      TypeHint::new(),
    )
  }
}

impl<S, Item, Err> RefCountCreator for InnerSharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
{
  type Connectable = SharedConnectableObservable<S, Item, Err>;
  fn new(connectable: Self::Connectable) -> RefCount<Self, Self::Connectable> {
    RefCount(
      InnerSharedRefCount(Arc::new(Mutex::new(Inner {
        connectable,
        connection: None,
      }))),
      TypeHint::new(),
    )
  }
}

impl<'a, Item, Err, S> Observable for LocalRefCount<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err, S> LocalObservable<'a> for LocalRefCount<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err> + Clone,
  S::Unsub: Clone,
  Item: Clone + 'a,
  Err: Clone + 'a,
{
  type Unsub = RefCountSubscription<LocalSubject<'a, Item, Err>, S::Unsub>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    let mut inner = (self.0).0.borrow_mut();
    inner.connectable.clone().actual_subscribe(observer);
    if inner.connection.is_none() {
      inner.connection = Some(inner.connectable.clone().connect());
    }
    let connection = inner.connection.as_ref().unwrap().clone();
    RefCountSubscription {
      subscription: inner.connectable.subject.clone(),
      connection,
    }
  }
}

impl<Item, Err, S> Observable for SharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
}

impl<Item, Err, S> SharedObservable for SharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err> + Clone,
  S::Unsub: Clone,
  Item: Clone + Send + Sync + 'static,
  Err: Clone + Send + Sync + 'static,
{
  type Unsub = RefCountSubscription<SharedSubject<Item, Err>, S::Unsub>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let mut inner = (self.0).0.lock().unwrap();
    inner.connectable.clone().actual_subscribe(observer);
    if inner.connection.is_none() {
      inner.connection = Some(inner.connectable.clone().connect());
    }
    let connection = inner.connection.as_ref().unwrap().clone();
    RefCountSubscription {
      subscription: inner.connectable.subject.clone(),
      connection,
    }
  }
}

#[derive(Clone)]
pub struct RefCountSubscription<S, C> {
  subscription: S,
  connection: C,
}

impl<S, C> SubscriptionLike for RefCountSubscription<S, C>
where
  S: TearDownSize,
  C: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self.subscription.unsubscribe();
    if self.subscription.teardown_size() == 0 {
      self.connection.unsubscribe();
    }
  }

  #[inline(always)]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn smoke() {
    let mut accept1 = 0;
    let mut accept2 = 0;
    {
      let ref_count = observable::of(1).publish().ref_count();
      ref_count.clone().subscribe(|v| accept1 = v);
      ref_count.clone().subscribe(|v| accept2 = v);
    }

    assert_eq!(accept1, 1);
    assert_eq!(accept2, 0);
  }

  #[test]
  fn auto_unsubscribe() {
    let mut accept1 = 0;
    let mut accept2 = 0;
    {
      let mut subject = LocalSubject::new();
      let ref_count = subject.clone().publish().ref_count();
      let mut s1 = ref_count.clone().subscribe(|v| accept1 = v);
      let mut s2 = ref_count.clone().subscribe(|v| accept2 = v);
      subject.next(1);
      s1.unsubscribe();
      s2.unsubscribe();
      subject.next(2);
    }

    assert_eq!(accept1, 1);
    assert_eq!(accept2, 1);
  }

  #[test]
  fn fork_and_shared() {
    observable::of(1).publish().ref_count().subscribe(|_| {});

    SharedSubject::new()
      .publish()
      .ref_count()
      .into_shared()
      .subscribe(|_: i32| {});

    observable::of(1)
      .publish()
      .ref_count()
      .into_shared()
      .subscribe(|_| {});

    observable::of(1)
      .into_shared()
      .publish()
      .ref_count()
      .into_shared()
      .subscribe(|_| {});
    observable::of(1)
      .into_shared()
      .publish()
      .ref_count()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_ref_count);

  fn bench_ref_count(b: &mut bencher::Bencher) { b.iter(smoke) }
}
