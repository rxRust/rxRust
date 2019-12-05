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
  Connect, LocalConnectableObservable, SharedConnectableObservable,
};
use crate::prelude::*;
use crate::subject::{LocalSubject, SharedSubject};
use crate::util::unwrap_rc_ref_cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

enum Source<C, S> {
  Connectable(C),
  Subject(S),
}

struct Inner<C, U>
where
  C: Fork,
{
  source: Source<C, C::Output>,
  connection: Option<U>,
}

type LocalRefConnectable<'a, O, Item, Err, U> =
  Rc<RefCell<Inner<LocalConnectableObservable<'a, O, Item, Err>, U>>>;
pub struct LocalRefCount<'a, O, Item, Err, U>(
  LocalRefConnectable<'a, O, Item, Err, U>,
);

type SharedRefConnectable<O, Item, Err, U> =
  Arc<Mutex<Inner<SharedConnectableObservable<O, Item, Err>, U>>>;
pub struct SharedRefCount<O, Item, Err, U>(
  SharedRefConnectable<O, Item, Err, U>,
);

pub(crate) fn local<'a, O, Item, Err>(
  connectable: LocalConnectableObservable<'a, O, Item, Err>,
) -> LocalRefCount<'a, O, Item, Err, O::Unsub>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<LocalSubject<'a, Item, Err>, LocalSubscription>,
  >,
{
  LocalRefCount(Rc::new(RefCell::new(Inner {
    source: Source::Connectable(connectable),
    connection: None,
  })))
}

pub(crate) fn shared<O, Item, Err>(
  connectable: SharedConnectableObservable<O, Item, Err>,
) -> SharedRefCount<O, Item, Err, O::Unsub>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<SharedSubject<Item, Err>, SharedSubscription>,
  >,
{
  SharedRefCount(Arc::new(Mutex::new(Inner {
    source: Source::Connectable(connectable),
    connection: None,
  })))
}

impl<'a, O, Item, Err, U> Fork for LocalRefCount<'a, O, Item, Err, U> {
  type Output = Self;
  #[inline(always)]
  fn fork(&self) -> Self::Output { LocalRefCount(self.0.clone()) }
}

impl<O, Item, Err, U> Fork for SharedRefCount<O, Item, Err, U> {
  type Output = Self;
  #[inline(always)]
  fn fork(&self) -> Self::Output { SharedRefCount(self.0.clone()) }
}

macro raw_subscribe($inner: ident, $subscriber: ident) {{
  let source = &mut $inner.source;
  let subscription = match source {
    Source::Connectable(c) => {
      let subject = c.fork();
      let c = std::mem::replace(source, Source::Subject(subject.fork()));
      let subscription = subject.raw_subscribe($subscriber);
      if let Source::Connectable(c) = c {
        $inner.connection = Some(c.connect());
      }
      subscription
    }
    Source::Subject(s) => s.fork().raw_subscribe($subscriber),
  };
  let connection = $inner.connection.as_ref().unwrap().clone();
  RefCountSubscription {
    subscription,
    connection,
  }
}}

impl<'a, O, Item, Err, SO>
  RawSubscribable<Item, Err, Subscriber<SO, LocalSubscription>>
  for LocalRefCount<'a, O, Item, Err, O::Unsub>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<LocalSubject<'a, Item, Err>, LocalSubscription>,
  >,
  SO: Observer<Item, Err> + 'a,
  O::Unsub: Clone,
{
  type Unsub = RefCountSubscription<LocalSubscription, O::Unsub>;
  fn raw_subscribe(
    self,
    subscriber: Subscriber<SO, LocalSubscription>,
  ) -> Self::Unsub {
    let mut inner = self.0.borrow_mut();
    raw_subscribe!(inner, subscriber)
  }
}

impl<O, Item, Err, SO, SU> RawSubscribable<Item, Err, Subscriber<SO, SU>>
  for SharedRefCount<O, Item, Err, O::Unsub>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<SharedSubject<Item, Err>, SharedSubscription>,
  >,
  SO: IntoShared,
  SU: IntoShared<Shared = SharedSubscription>,
  SO::Shared: Observer<Item, Err> + IntoShared,
  <SO::Shared as IntoShared>::Shared: Observer<Item, Err>,
  O::Unsub: Clone,
{
  type Unsub = RefCountSubscription<SharedSubscription, O::Unsub>;
  fn raw_subscribe(self, subscriber: Subscriber<SO, SU>) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let mut inner = self.0.lock().unwrap();
    raw_subscribe!(inner, subscriber)
  }
}

pub struct RefCountSubscription<S, C> {
  subscription: S,
  connection: C,
}

macro subscription_impl($ty: ty) {
  impl<C> SubscriptionLike for RefCountSubscription<$ty, C>
  where
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
}

subscription_impl!(LocalSubscription);
subscription_impl!(SharedSubscription);

impl<'a, O, Item, Err, U> IntoShared for LocalRefCount<'a, O, Item, Err, U>
where
  O: IntoShared,
  U: IntoShared,
  Item: 'static,
  Err: 'static,
{
  type Shared = SharedRefCount<O::Shared, Item, Err, U::Shared>;
  fn to_shared(self) -> Self::Shared {
    let ref_count = unwrap_rc_ref_cell(
      self.0,
      "Cannot convert a `LocalRefCount` to `SharedRefCount` \
       when it referenced by other.",
    );
    if ref_count.connection.is_some() {
      panic!("Can not convert a connected RefCountOp into shared.");
    }
    let source = match ref_count.source {
      Source::Connectable(c) => Source::Connectable(c.to_shared()),
      Source::Subject(_) => {
        panic!("Can not convert a connected RefCountOp into shared.")
      }
    };

    SharedRefCount(Arc::new(Mutex::new(Inner {
      source,
      connection: None,
    })))
  }
}

impl<O, Item, Err, U> IntoShared for SharedRefCount<O, Item, Err, U>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

impl<O, Item, Err> SharedConnectableObservable<O, Item, Err> {
  pub fn ref_count(self) -> SharedRefCount<O, Item, Err, O::Unsub>
  where
    O: RawSubscribable<
      Item,
      Err,
      Subscriber<SharedSubject<Item, Err>, SharedSubscription>,
    >,
  {
    shared(self)
  }
}

impl<'a, O, Item, Err> LocalConnectableObservable<'a, O, Item, Err> {
  pub fn ref_count(self) -> LocalRefCount<'a, O, Item, Err, O::Unsub>
  where
    O: RawSubscribable<
      Item,
      Err,
      Subscriber<LocalSubject<'a, Item, Err>, LocalSubscription>,
    >,
  {
    local(self)
  }
}

#[test]
fn smoke() {
  use crate::ops::Publish;
  let mut accept1 = 0;
  let mut accept2 = 0;
  {
    let ref_count = observable::of(1).fork().publish().ref_count();
    ref_count.fork().subscribe(|v| accept1 = v);
    ref_count.fork().subscribe(|v| accept2 = v);
  }

  assert_eq!(accept1, 1);
  assert_eq!(accept2, 0);
}

#[test]
fn auto_unsubscribe() {
  use crate::ops::Publish;
  let mut accept1 = 0;
  let mut accept2 = 0;
  {
    let mut subject = Subject::local();
    let ref_count = subject.fork().publish().ref_count();
    let mut s1 = ref_count.fork().subscribe(|v| accept1 = v);
    let mut s2 = ref_count.fork().subscribe(|v| accept2 = v);
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
  use crate::ops::Publish;
  observable::of(1).publish().ref_count().subscribe(|_| {});

  LocalSubject::<'_, (), ()>::local()
    .publish()
    .ref_count()
    .to_shared()
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
    .subscribe(|_| {});
  observable::of(1)
    .publish()
    .to_shared()
    .ref_count()
    .to_shared()
    .to_shared()
    .subscribe(|_| {});
}
