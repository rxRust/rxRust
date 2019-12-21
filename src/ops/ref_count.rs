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
use crate::observable::{Connect, ConnectableObservable};
use crate::prelude::*;
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

type LocalRefConnectable<Source, Subject, U> =
  Rc<RefCell<Inner<ConnectableObservable<Source, Subject>, U>>>;
pub struct LocalRefCount<Source, Subject: Fork, U>(
  LocalRefConnectable<Source, Subject, U>,
);

type SharedRefConnectable<Source, Subject, U> =
  Arc<Mutex<Inner<ConnectableObservable<Source, Subject>, U>>>;
pub struct SharedRefCount<Source, Subject: Fork, U>(
  SharedRefConnectable<Source, Subject, U>,
);

pub(crate) fn local<S, O, U>(
  connectable: ConnectableObservable<S, Subject<O, U>>,
) -> LocalRefCount<S, Subject<O, U>, S::Unsub>
where
  S: RawSubscribable<Subscriber<O, U>>,
  Subject<O, U>: Fork,
{
  LocalRefCount(Rc::new(RefCell::new(Inner {
    source: Source::Connectable(connectable),
    connection: None,
  })))
}

impl<Source, Subject: Fork, U> Fork for LocalRefCount<Source, Subject, U> {
  type Output = Self;
  #[inline(always)]
  fn fork(&self) -> Self::Output { LocalRefCount(self.0.clone()) }
}

impl<Source, Subject: Fork, U> Fork for SharedRefCount<Source, Subject, U> {
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

impl<S, O, U, Sub> RawSubscribable<Sub>
  for LocalRefCount<S, Subject<O, U>, S::Unsub>
where
  S: RawSubscribable<Subscriber<O, U>>,
  Subject<O, U>: RawSubscribable<Sub, Unsub = U> + Fork<Output = Subject<O, U>>,
  S::Unsub: Clone,
  U: TearDownSize + 'static,
{
  type Unsub = RefCountSubscription<U, S::Unsub>;
  fn raw_subscribe(self, subscriber: Sub) -> Self::Unsub {
    let mut inner = self.0.borrow_mut();
    raw_subscribe!(inner, subscriber)
  }
}

impl<S, O, U, Sub> RawSubscribable<Sub>
  for SharedRefCount<S, Subject<O, U>, S::Unsub>
where
  S: RawSubscribable<Subscriber<O, U>>,
  Subject<O, U>:
    RawSubscribable<Sub::Shared, Unsub = U> + Fork<Output = Subject<O, U>>,
  U: TearDownSize + 'static,
  S::Unsub: Clone,
  Sub: IntoShared,
{
  type Unsub = RefCountSubscription<U, S::Unsub>;
  fn raw_subscribe(self, subscriber: Sub) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let mut inner = self.0.lock().unwrap();
    raw_subscribe!(inner, subscriber)
  }
}

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

impl<S, Subject, U> IntoShared for LocalRefCount<S, Subject, U>
where
  S: IntoShared,
  Subject: IntoShared + Fork<Output = Subject>,
  Subject::Shared: Fork<Output = Subject::Shared>,
  U: IntoShared,
{
  type Shared = SharedRefCount<S::Shared, Subject::Shared, U::Shared>;
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

impl<S, Subject, U> IntoShared for SharedRefCount<S, Subject, U>
where
  Self: Send + Sync + 'static,
  Subject: Fork,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

impl<Source, O, U> ConnectableObservable<Source, Subject<O, U>> {
  pub fn ref_count(self) -> LocalRefCount<Source, Subject<O, U>, Source::Unsub>
  where
    Source: RawSubscribable<Subscriber<O, U>>,
    Subject<O, U>: Fork,
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

#[test]
fn filter() {
  use crate::ops::{FilterMap, Publish};
  let mut subject = Subject::local_mut_ref();

  subject
    .fork()
    .filter_map::<_, &mut i32, _>(Some)
    .publish_mut_ref()
    .subscribe_err(|_: &mut i32| {}, |_: &mut i32| {});

  subject.next(&mut 1);
  subject.error(&mut 2);
}
