/// Make a ConnectableObservable behave like a ordinary observable and automates
/// the way you can connect to it.
///
/// Internally it counts the subscriptions to the observable and subscribes
/// (only once) to the source if the number of subscriptions is larger than 0.
/// If the number of subscriptions is smaller than 1, it unsubscribes from the
/// source. This way you can make sure that everything before the published
/// refCount has only a single subscription independently of the number of
/// subscribers to the target observable.
///
/// Note that using the share operator is exactly the same as using the publish
/// operator (making the observable hot) and the refCount operator in a
/// sequence.
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

#[derive(Clone)]
pub struct LocalRefCount<'a, S, Item, Err>(
  Rc<RefCell<Inner<LocalConnectableObservable<'a, S, Item, Err>, S::Unsub>>>,
)
where
  S: Observable<'a, Item = Item, Err = Err>;

#[derive(Clone)]
pub struct SharedRefCount<S, Item, Err>(
  Arc<Mutex<Inner<SharedConnectableObservable<S, Item, Err>, S::Unsub>>>,
)
where
  S: SharedObservable<Item = Item, Err = Err>;

impl<'a, S, Item, Err> LocalRefCount<'a, S, Item, Err>
where
  S: Observable<'a, Item = Item, Err = Err>,
{
  pub fn new(
    connectable: LocalConnectableObservable<'a, S, Item, Err>,
  ) -> Self {
    LocalRefCount(Rc::new(RefCell::new(Inner {
      connectable,
      connection: None,
    })))
  }
}

impl<S, Item, Err> SharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
{
  pub fn new(connectable: SharedConnectableObservable<S, Item, Err>) -> Self {
    SharedRefCount(Arc::new(Mutex::new(Inner {
      connectable,
      connection: None,
    })))
  }
}

impl<'a, Item, Err, S> Observable<'a> for LocalRefCount<'a, S, Item, Err>
where
  S: Observable<'a, Item = Item, Err = Err> + Clone,
  S::Unsub: Clone,
  Item: Copy + 'a,
  Err: Copy + 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsub = RefCountSubscription<LocalSubscription, S::Unsub>;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let mut inner = self.0.borrow_mut();
    if !inner.connectable.subject.is_subscribed() {
      inner.connection = Some(inner.connectable.clone().connect());
    }
    inner.connectable.clone().actual_subscribe(subscriber);
    let connection = inner.connection.as_ref().unwrap().clone();
    RefCountSubscription {
      subscription: inner.connectable.subject.subscription.clone(),
      connection,
    }
  }
}

impl<Item, Err, S> SharedObservable for SharedRefCount<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err> + Clone,
  S::Unsub: Clone,
  Item: Copy + Send + Sync + 'static,
  Err: Copy + Send + Sync + 'static,
{
  type Item = Item;
  type Err = Err;
  type Unsub = RefCountSubscription<SharedSubscription, S::Unsub>;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let mut inner = self.0.lock().unwrap();
    if !inner.connectable.subject.is_subscribed() {
      inner.connection = Some(inner.connectable.clone().connect());
    }
    inner.connectable.clone().actual_subscribe(subscriber);
    let connection = inner.connection.as_ref().unwrap().clone();
    RefCountSubscription {
      subscription: inner.connectable.subject.subscription.clone(),
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

  #[inline(always)]
  fn inner_addr(&self) -> *const () { self.subscription.inner_addr() }
}

#[test]
fn smoke() {
  let mut accept1 = 0;
  let mut accept2 = 0;
  {
    let ref_count = Observable::publish(observable::of(1)).ref_count();
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
    let mut subject = Subject::local();
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

  LocalSubject::local()
    .publish()
    .ref_count()
    .to_shared()
    .subscribe(|_: i32| {});

  observable::of(1)
    .to_shared()
    .publish()
    .ref_count()
    .subscribe(|_| {});

  observable::of(1)
    .publish()
    .to_shared()
    .ref_count()
    .subscribe(|_| {});
  observable::of(1)
    .publish()
    .to_shared()
    .ref_count()
    .to_shared()
    .to_shared()
    .subscribe(|_| {});
}

#[test]
#[should_panic]
fn convert_local_ref_count_to_shared_should_panic() {
  let ref_count = Subject::local().clone().publish().ref_count();
  ref_count.clone().subscribe(|_: i32| {});
  ref_count.to_shared();
}
