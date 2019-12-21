use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;
use crate::util;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Subject<O, S> {
  pub(crate) observers: O,
  pub(crate) subscription: S,
}

pub type LocalObserver<P> = Rc<RefCell<Vec<P>>>;

type LSubject<V> = Subject<Rc<RefCell<V>>, LocalSubscription>;

pub struct LocalSubjectVec<'a, Item, Err>(
  Vec<Box<dyn Publisher<Item, Err> + 'a>>,
);
pub type LocalSubject<'a, Item, Err> = LSubject<LocalSubjectVec<'a, Item, Err>>;

#[allow(clippy::type_complexity)]
pub struct LocalSubjectMutRefVec<'a, Item, Err>(
  Vec<Box<dyn for<'r> Publisher<&'r mut Item, &'r mut Err> + 'a>>,
);
pub type LocalSubjectMutRef<'a, Item, Err> =
  LSubject<LocalSubjectMutRefVec<'a, Item, Err>>;

#[allow(clippy::type_complexity)]
pub struct LocalSubjectMutRefItemVec<'a, Item, Err>(
  Vec<Box<dyn for<'r> Publisher<&'r mut Item, Err> + 'a>>,
);
pub type LocalSubjectMutRefItem<'a, Item, Err> =
  LSubject<LocalSubjectMutRefItemVec<'a, Item, Err>>;

#[allow(clippy::type_complexity)]
pub struct LocalSubjectMutRefErrVec<'a, Item, Err>(
  Vec<Box<dyn for<'r> Publisher<Item, &'r mut Err> + 'a>>,
);
pub type LocalSubjectMutRefErr<'a, Item, Err> =
  LSubject<LocalSubjectMutRefErrVec<'a, Item, Err>>;

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

macro local_subject_create($ty: ident) {
  Subject {
    observers: Rc::new(RefCell::new($ty(vec![]))),
    subscription: LocalSubscription::default(),
  }
}

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  pub fn local() -> Self { local_subject_create!(LocalSubjectVec) }
}

impl<'a, Item, Err> LocalSubjectMutRef<'a, Item, Err> {
  pub fn local_mut_ref() -> Self {
    local_subject_create!(LocalSubjectMutRefVec)
  }
}

impl<'a, Item, Err> LocalSubjectMutRefItem<'a, Item, Err> {
  #[inline(always)]
  pub fn local_mut_ref_item() -> Self {
    local_subject_create!(LocalSubjectMutRefItemVec)
  }
}

impl<'a, Item, Err> LocalSubjectMutRefErr<'a, Item, Err> {
  #[inline(always)]
  pub fn local_mut_ref_err() -> Self {
    local_subject_create!(LocalSubjectMutRefErrVec)
  }
}

impl<Item, Err> SharedSubject<Item, Err> {
  pub fn shared() -> Self {
    Subject {
      observers: Arc::new(Mutex::new(vec![])),
      subscription: SharedSubscription::default(),
    }
  }
}
impl<Item, Err> IntoShared for SharedSubject<Item, Err>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<'a, Item, Err> IntoShared for LocalSubject<'a, Item, Err>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Subject<SharedPublishers<Item, Err>, SharedSubscription>;
  fn to_shared(self) -> Self::Shared {
    let Self {
      observers,
      subscription,
    } = self;
    let observers = util::unwrap_rc_ref_cell(
      observers,
      "Cannot convert a `LocalSubscription` to `SharedSubscription` \
       when it referenced by other.",
    );
    let observers = if observers.0.is_empty() {
      Arc::new(Mutex::new(vec![]))
    } else {
      panic!(
        "Cannot convert a `LocalSubscription` to `SharedSubscription` \
         when it subscribed."
      )
    };
    let subscription = subscription.to_shared();
    Subject {
      observers,
      subscription,
    }
  }
}

macro local_subject_raw_subscribe_impl($o: ident,$u: ident) {
  type Unsub = $u;
  fn raw_subscribe(mut self, subscriber: Subscriber<$o, $u>) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.borrow_mut().0.push(Box::new(subscriber));
    subscription
  }
}
impl<'a, Item, Err, O, U> RawSubscribable<Subscriber<O, U>>
  for LocalSubject<'a, Item, Err>
where
  O: Observer<Item, Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_raw_subscribe_impl!(O, U);
}

impl<'a, Item, Err, O, U> RawSubscribable<Subscriber<O, U>>
  for LocalSubjectMutRefItem<'a, Item, Err>
where
  O: for<'r> Observer<&'r mut Item, Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_raw_subscribe_impl!(O, U);
}

impl<'a, Item, Err, O, U> RawSubscribable<Subscriber<O, U>>
  for LocalSubjectMutRefErr<'a, Item, Err>
where
  O: for<'r> Observer<Item, &'r mut Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_raw_subscribe_impl!(O, U);
}

impl<'a, Item, Err, O, U> RawSubscribable<Subscriber<O, U>>
  for LocalSubjectMutRef<'a, Item, Err>
where
  O: for<'r> Observer<&'r mut Item, &'r mut Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_raw_subscribe_impl!(O, U);
}

impl<Item, Err, O, S> RawSubscribable<Subscriber<O, S>>
  for SharedSubject<Item, Err>
where
  S: IntoShared,
  O: IntoShared,
  O::Shared: Observer<Item, Err>,
  S::Shared: SubscriptionLike + Clone + 'static,
{
  type Unsub = S::Shared;
  fn raw_subscribe(mut self, subscriber: Subscriber<O, S>) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.lock().unwrap().push(Box::new(subscriber));
    subscription
  }
}

impl<O, S> SubscriptionLike for Subject<O, S>
where
  S: SubscriptionLike,
{
  #[inline]
  fn unsubscribe(&mut self) { self.subscription.unsubscribe() }
  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
  #[inline]
  fn inner_addr(&self) -> *const () { self.subscription.inner_addr() }
}

impl<O, S> Fork for Subject<O, S>
where
  Self: Clone,
{
  type Output = Self;
  fn fork(&self) -> Self::Output { self.clone() }
}

impl<Item, T> ObserverNext<Item> for Vec<T>
where
  Item: Copy,
  T: ObserverNext<Item> + SubscriptionLike,
{
  fn next(&mut self, value: Item) {
    self.drain_filter(|subscriber| {
      subscriber.next(value);
      subscriber.is_closed()
    });
  }
}

impl<Err, T> ObserverError<Err> for Vec<T>
where
  Err: Copy,
  T: ObserverError<Err> + SubscriptionLike,
{
  fn error(&mut self, err: Err) {
    self.iter_mut().for_each(|subscriber| {
      subscriber.error(err);
    });
    self.clear();
  }
}

impl<T> ObserverComplete for Vec<T>
where
  T: ObserverComplete,
{
  fn complete(&mut self) {
    self.iter_mut().for_each(|subscriber| {
      subscriber.complete();
    });
    self.clear();
  }
}

/// After rust fixed [this bug](https://github.com/rust-lang/rust/issues/48869)
/// we needn't split local subject to four vec collection type, and implement
/// `Observer` for them one by one.LocalSubjectMutRefErrVec
///
/// Just implement specialization versions of `Observer` for `Vec<T>`, like this
/// ```ignore
/// impl<Item, T> ObserverNext<&mut Item> for Vec<T>
/// where
///   T: for<'r> ObserverNext<&'r mut Item> + SubscriptionLike,
/// {
///   fn next(&mut self, value: &mut Item) {
///     self.drain_filter(|subscriber| {
///       subscriber.next(value);
///       subscriber.is_closed()
///     });
///   }
/// }
/// ```
///
/// specialization impl for local subject.

macro impl_local_subject_vec_observer_next($item: ty) {
  fn next(&mut self, value: $item) {
    self.0.drain_filter(|subscriber| {
      subscriber.next(value);
      subscriber.is_closed()
    });
  }
}

macro impl_local_subject_vec_observer_error($err: ty) {
  fn error(&mut self, err: $err) {
      self.0.iter_mut().for_each(|subscriber| {
        subscriber.error(err);
      });
      self.0.clear();
    }
}
macro impl_local_subject_vec_observer_complete() {
  fn complete(&mut self) {
      self.0.iter_mut().for_each(|subscriber| {
        subscriber.complete();
      });
      self.0.clear();
    }
}

impl<'a, Item, Err> ObserverNext<Item> for LocalSubjectVec<'a, Item, Err>
where
  Item: Copy,
{
  impl_local_subject_vec_observer_next!(Item);
}
impl<'a, Item, Err> ObserverError<Err> for LocalSubjectVec<'a, Item, Err>
where
  Err: Copy,
{
  impl_local_subject_vec_observer_error!(Err);
}
impl<'a, Item, Err> ObserverComplete for LocalSubjectVec<'a, Item, Err> {
  impl_local_subject_vec_observer_complete!();
}

impl<'a, Item, Err> ObserverNext<&mut Item>
  for LocalSubjectMutRefItemVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_next!(&mut Item);
}
impl<'a, Item, Err> ObserverError<Err>
  for LocalSubjectMutRefItemVec<'a, Item, Err>
where
  Err: Copy,
{
  impl_local_subject_vec_observer_error!(Err);
}
impl<'a, Item, Err> ObserverComplete
  for LocalSubjectMutRefItemVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_complete!();
}

impl<'a, Item, Err> ObserverNext<Item>
  for LocalSubjectMutRefErrVec<'a, Item, Err>
where
  Item: Copy,
{
  impl_local_subject_vec_observer_next!(Item);
}
impl<'a, Item, Err> ObserverError<&mut Err>
  for LocalSubjectMutRefErrVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_error!(&mut Err);
}
impl<'a, Item, Err> ObserverComplete
  for LocalSubjectMutRefErrVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_complete!();
}

impl<'a, Item, Err> ObserverNext<&mut Item>
  for LocalSubjectMutRefVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_next!(&mut Item);
}
impl<'a, Item, Err> ObserverError<&mut Err>
  for LocalSubjectMutRefVec<'a, Item, Err>
{
  impl_local_subject_vec_observer_error!(&mut Err);
}
impl<'a, Item, Err> ObserverComplete for LocalSubjectMutRefVec<'a, Item, Err> {
  impl_local_subject_vec_observer_complete!();
}

impl<Item, S, O> ObserverNext<Item> for Subject<O, S>
where
  O: ObserverNext<Item>,
  S: SubscriptionLike,
{
  fn next(&mut self, value: Item) {
    if !self.subscription.is_closed() {
      self.observers.next(value)
    }
  }
}

impl<Err, S, O> ObserverError<Err> for Subject<O, S>
where
  O: ObserverError<Err>,
  S: SubscriptionLike,
{
  fn error(&mut self, err: Err) {
    if !self.subscription.is_closed() {
      self.observers.error(err);
      self.subscription.unsubscribe();
    };
  }
}

impl<S, O> ObserverComplete for Subject<O, S>
where
  O: ObserverComplete,
  S: SubscriptionLike,
{
  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      self.observers.complete();
      self.subscription.unsubscribe();
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn emit_ref() {
    // emit ref
    let mut subject: LocalSubject<'_, _, ()> = Subject::local();
    subject.next(&1);

    // emit mut ref
    let mut subject = Subject::local_mut_ref_item();
    subject.fork().subscribe(|_: &mut _| {});
    subject.next(&mut 1);
  }
  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::local();
      broadcast.fork().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::local();
    broadcast
      .fork()
      .subscribe_err(|_: i32| {}, |e: _| panic!(e));
    broadcast.next(1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = Subject::local();
      subject.fork().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn fork_and_shared() {
    let subject = Subject::shared();
    subject
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_: &()| {});
  }

  #[test]
  fn empty_local_subject_can_convert_to_shared() {
    use crate::{ops::ObserveOn, scheduler::Schedulers};
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let mut subject = Subject::local().to_shared();
    subject.fork().observe_on(Schedulers::NewThread).subscribe(
      move |v: i32| {
        *value.lock().unwrap() = v;
      },
    );

    subject.next(100);
    std::thread::sleep(std::time::Duration::from_millis(1));

    assert_eq!(*c_v.lock().unwrap(), 100);
  }
}
