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

pub type LocalSubject<'a, Item, Err> =
  Subject<LocalObserver<Box<dyn Publisher<Item, Err> + 'a>>, LocalSubscription>;

pub type LocalSubjectMutRef<'a, Item, Err> = Subject<
  LocalObserver<Box<dyn for<'r> Publisher<&'r mut Item, &'r mut Err> + 'a>>,
  LocalSubscription,
>;

pub type LocalSubjectMutRefItem<'a, Item, Err> = Subject<
  LocalObserver<Box<dyn for<'r> Publisher<&'r mut Item, Err> + 'a>>,
  LocalSubscription,
>;

pub type LocalSubjectMutRefErr<'a, Item, Err> = Subject<
  LocalObserver<Box<dyn for<'r> Publisher<Item, &'r mut Err> + 'a>>,
  LocalSubscription,
>;

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  #[inline(always)]
  pub fn local() -> Self { Subject::new() }
}

impl<'a, Item, Err> LocalSubjectMutRef<'a, Item, Err> {
  #[inline(always)]
  pub fn local_mut_ref() -> Self { Subject::new() }
}

impl<'a, Item, Err> LocalSubjectMutRefItem<'a, Item, Err> {
  #[inline(always)]
  pub fn local_mut_ref_item() -> Self { Subject::new() }
}

impl<'a, Item, Err> LocalSubjectMutRefErr<'a, Item, Err> {
  #[inline(always)]
  pub fn local_mut_ref_err() -> Self { Subject::new() }
}

impl<P> Subject<LocalObserver<P>, LocalSubscription> {
  fn new() -> Self {
    Subject {
      observers: Rc::new(RefCell::new(vec![])),
      subscription: LocalSubscription::default(),
    }
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
    let observers = if observers.is_empty() {
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
    self.observers.borrow_mut().push(Box::new(subscriber));
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

/// By default, `Subject` can only emit item which implemented `Copy`,
/// that because subject is a multicast stream, need pass item to multi
/// subscriber.
/// But `Copy` bound is not perfect to subject. Like `&mut T` should be emitted
/// by `Subject`, because `Subject` just immediately pass it to subscribers and
/// never alias it, but `&mut T` is not `Copy`.
/// So we introduced `SubjectCopy`, it's only use to support `Item` be passed
/// to many subscriber in `Subject`. The drawback is `SubjectCopy` is not auto
/// implement for a type when the type is implemented `Copy`. Just use macro
/// `impl_subject_copy_for_copy!` to impl `SubjectCopy` for your type after
/// implemented `Copy`.
///
/// # Example
/// ```
/// # use rxrust::prelude::*;
/// # use rxrust::subject::impl_subject_copy_for_copy;
///
/// #[derive(Clone)]
/// struct CustomType;
///
/// impl Copy for CustomType {}
/// impl_subject_copy_for_copy!(CustomType);
///
/// // can pass `&mut CustomType` now.
/// let mut ct = CustomType;
///
/// let mut subject = Subject::local();
/// subject.next(&mut ct);
/// subject.error(());
/// ```
pub trait SubjectCopy {
  fn copy(&self) -> Self;
}

/// todo: maybe we can support SubjectCopy like below.
///
/// impl<T: Copy> SubjectCopy for T {
/// #[inline]
///   fn copy(&self) -> Self { self.clone() }
/// }
///
/// impl<T> SubjectCopy for &mut T {
///   #[inline]
///   fn copy(&self) -> Self { unsafe { std::mem::transmute_copy(self) } }
/// }
///
/// but code like these can't compile, so we'll implement `SubjectCopy` for
/// primitive types.
impl<T> SubjectCopy for &mut T {
  #[inline]
  fn copy(&self) -> Self { unsafe { std::mem::transmute_copy(self) } }
}

pub macro impl_subject_copy_for_copy($t: ty) {
  impl SubjectCopy for $t {
    #[inline]
    fn copy(&self) -> Self { *self }
  }
}
mod subject_copy_impls {
  use super::{impl_subject_copy_for_copy, SubjectCopy};

  macro impl_subject_copys($($t:ty )*) {
    $(impl_subject_copy_for_copy!($t);)*
  }

  impl_subject_copys! {
      usize u8 u16 u32 u64 u128
      isize i8 i16 i32 i64 i128
      f32 f64
      bool char
  }

  impl_subject_copy_for_copy!(!);
  impl_subject_copy_for_copy!(());

  impl<T: ?Sized> SubjectCopy for *const T {
    #[inline]
    fn copy(&self) -> Self { *self }
  }

  impl<T: ?Sized> SubjectCopy for *mut T {
    #[inline]
    fn copy(&self) -> Self { *self }
  }

  impl<T: ?Sized> SubjectCopy for &T {
    #[inline]
    fn copy(&self) -> Self { *self }
  }
}

impl<Item, T> ObserverNext<Item> for Vec<T>
where
  Item: SubjectCopy,
  T: ObserverNext<Item> + SubscriptionLike,
{
  fn next(&mut self, value: Item) {
    self.drain_filter(|subscriber| {
      subscriber.next(value.copy());
      subscriber.is_closed()
    });
  }
}

impl<Err, T> ObserverError<Err> for Vec<T>
where
  Err: SubjectCopy,
  T: ObserverError<Err> + SubscriptionLike,
{
  fn error(&mut self, err: Err) {
    self.iter_mut().for_each(|subscriber| {
      subscriber.error(err.copy());
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

    let ds = Subject::local_mut_ref_item();
    subject.fork().raw_subscribe(Subscriber {
      observer: ds.observers,
      subscription: ds.subscription,
    });
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
