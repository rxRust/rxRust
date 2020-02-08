use crate::observer::{
  observer_complete_proxy_impl, ObserverComplete, ObserverError, ObserverNext,
};
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

#[derive(Clone, Copy)]
pub struct SubjectValue<T>(T);

pub struct SubjectMutRefValue<T>(*mut T);

impl<T> Clone for SubjectMutRefValue<T> {
  fn clone(&self) -> Self { *self }
}

impl<T> Copy for SubjectMutRefValue<T> {}

type RcBoxPublisher<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;
pub struct LocalSubjectObserver<'a, Item, Err>(RcBoxPublisher<'a, Item, Err>);
pub type LocalSubject<'a, Item, Err> =
  Subject<LocalSubjectObserver<'a, Item, Err>, LocalSubscription>;

type SharedPublishers<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  pub fn local() -> Self {
    Subject {
      observers: LocalSubjectObserver(Rc::new(RefCell::new(vec![]))),
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

impl<'a, Item, Err> IntoShared
  for LocalSubject<'a, SubjectValue<Item>, SubjectValue<Err>>
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
      observers.0,
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

macro local_subject_actual_subscribe_impl($o: ident,$u: ident) {
  type Unsub = $u;
  fn actual_subscribe(mut self, subscriber: Subscriber<$o, $u>) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self
      .observers
      .0.borrow_mut()
      .push(Box::new(LocalSubjectSubscriber(subscriber)));
    subscription
  }
}

impl<'a, Item: Copy, Err: Copy, O, U> Observable<O, U>
  for LocalSubject<'a, SubjectValue<Item>, SubjectValue<Err>>
where
  O: Observer<Item, Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_actual_subscribe_impl!(O, U);
}

impl<'a, Item, Err: Copy, O, U> Observable<O, U>
  for LocalSubject<'a, SubjectMutRefValue<Item>, SubjectValue<Err>>
where
  O: for<'r> Observer<&'r mut Item, Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_actual_subscribe_impl!(O, U);
}

impl<'a, Item: Copy, Err, O, U> Observable<O, U>
  for LocalSubject<'a, SubjectValue<Item>, SubjectMutRefValue<Err>>
where
  O: for<'r> Observer<Item, &'r mut Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_actual_subscribe_impl!(O, U);
}

impl<'a, Item, Err, O, U> Observable<O, U>
  for LocalSubject<'a, SubjectMutRefValue<Item>, SubjectMutRefValue<Err>>
where
  O: for<'r> Observer<&'r mut Item, &'r mut Err> + 'a,
  U: SubscriptionLike + Clone + 'static,
{
  local_subject_actual_subscribe_impl!(O, U);
}

impl<Item, Err, O, U> Observable<O, U> for SharedSubject<Item, Err>
where
  U: IntoShared + SubscriptionLike,
  O: IntoShared + Observer<Item, Err>,
  O::Shared: Observer<Item, Err>,
  U::Shared: SubscriptionLike + Clone + 'static,
{
  type Unsub = U::Shared;
  fn actual_subscribe(mut self, subscriber: Subscriber<O, U>) -> Self::Unsub {
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

struct LocalSubjectSubscriber<T>(T);

impl<Item, T> ObserverNext<SubjectValue<Item>> for LocalSubjectSubscriber<T>
where
  T: ObserverNext<Item>,
{
  #[inline(always)]
  fn next(&mut self, value: SubjectValue<Item>) { self.0.next(value.0) }
}

impl<Err, T> ObserverError<SubjectValue<Err>> for LocalSubjectSubscriber<T>
where
  T: ObserverError<Err>,
{
  #[inline(always)]
  fn error(&mut self, value: SubjectValue<Err>) { self.0.error(value.0) }
}

impl<Item, T> ObserverNext<SubjectMutRefValue<Item>>
  for LocalSubjectSubscriber<T>
where
  T: for<'r> ObserverNext<&'r mut Item>,
{
  #[inline(always)]
  fn next(&mut self, value: SubjectMutRefValue<Item>) {
    // unsafe introduce
    // this unsafe code is safe because we just use it to emit item by mut ref
    // in LocalSubject. LocalSubject just pass item to downstream one by one and
    // never alias the mut ref.
    self.0.next(unsafe { &mut (*value.0) })
  }
}

impl<Err, T> ObserverError<SubjectMutRefValue<Err>>
  for LocalSubjectSubscriber<T>
where
  T: for<'r> ObserverError<&'r mut Err>,
{
  #[inline(always)]
  fn error(&mut self, value: SubjectMutRefValue<Err>) {
    // unsafe introduce
    // this unsafe code is safe because we just use it to emit error by mut ref
    // in LocalSubject. LocalSubject just pass error to downstream one by one
    // and never alias the mut ref.
    self.0.error(unsafe { &mut (*value.0) })
  }
}

observer_complete_proxy_impl!(LocalSubjectSubscriber<T>, T, 0, <T> );

impl<T> SubscriptionLike for LocalSubjectSubscriber<T>
where
  T: SubscriptionLike,
{
  #[inline(always)]
  fn unsubscribe(&mut self) { self.0.unsubscribe(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { self.0.is_closed() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { self.0.inner_addr() }
}

impl<'a, Item: Copy, Err> ObserverNext<Item>
  for LocalSubjectObserver<'a, SubjectValue<Item>, Err>
{
  #[inline]
  fn next(&mut self, value: Item) { self.0.next(SubjectValue(value)) }
}

impl<'a, Item, Err> ObserverNext<&mut Item>
  for LocalSubjectObserver<'a, SubjectMutRefValue<Item>, Err>
{
  #[inline]
  fn next(&mut self, value: &mut Item) {
    self.0.next(SubjectMutRefValue(value))
  }
}

impl<'a, Item, Err: Copy> ObserverError<Err>
  for LocalSubjectObserver<'a, Item, SubjectValue<Err>>
{
  #[inline]
  fn error(&mut self, err: Err) { self.0.error(SubjectValue(err)) }
}

impl<'a, Item, Err> ObserverError<&mut Err>
  for LocalSubjectObserver<'a, Item, SubjectMutRefValue<Err>>
{
  #[inline]
  fn error(&mut self, err: &mut Err) { self.0.error(SubjectMutRefValue(err)) }
}

impl<'a, Item, Err> ObserverComplete for LocalSubjectObserver<'a, Item, Err> {
  #[inline]
  fn complete(&mut self) { self.0.complete(); }
}

impl<'a, Item, Err> Clone for LocalSubjectObserver<'a, Item, Err> {
  #[inline]
  fn clone(&self) -> Self { LocalSubjectObserver(self.0.clone()) }
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
  use crate::ops::FilterMap;

  #[test]
  fn emit_ref() {
    // emit ref
    let mut subject: LocalSubject<'_, _, ()> = Subject::local();
    subject.next(&1);

    let mut i = 1;
    // emit item by mut ref, emit error by value
    let mut subject = Subject::local();
    let _guard = subject.fork().subscribe_err(|v: &mut _| *v = 100, |_| {});
    subject.next(&mut i);
    subject.error(1);
    assert_eq!(i, 100);

    // emit item by value, emit error by mut ref
    let mut subject = Subject::local();
    subject.fork().subscribe_err(|_| {}, |_: &mut _| {});
    subject.next(1);
    subject.error(&mut 1);
    // emit item by mut ref and emit error by mut ref
    let mut subject = Subject::local();
    subject.fork().subscribe_err(|_: &mut _| {}, |_: &mut _| {});
    subject.next(&mut 1);
    subject.error(&mut 1);
  }

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::local();
      let _guard = broadcast.fork().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::local();
    let _guard = broadcast
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
    let _guard = subject.fork().observe_on(Schedulers::NewThread).subscribe(
      move |v: i32| {
        *value.lock().unwrap() = v;
      },
    );

    subject.next(100);
    std::thread::sleep(std::time::Duration::from_millis(1));

    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn emit_mut_ref_life_time() {
    let mut i = 1;
    {
      // emit mut ref
      let mut subject = Subject::local();
      let _guard = subject
        .fork()
        .filter_map((|v| Some(v)) as for<'r> fn(&'r mut _) -> Option<&'r mut _>)
        .subscribe(|_: &mut i32| {
          i = 100;
        });
      subject.next(&mut 1);
    }
    assert_eq!(i, 100);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = Subject::local();
    let local2 = Subject::local();
    local.fork().actual_subscribe(Subscriber {
      observer: local2.observers,
      subscription: local2.subscription,
    });
    local.next(1);
    local.error(2);
  }

  #[test]
  #[should_panic]
  fn convert_subscribed_local_subject_to_shared_should_panic() {
    let subject = Subject::local();
    subject.fork().subscribe(|_: i32| {});
    subject.to_shared();
  }
}
