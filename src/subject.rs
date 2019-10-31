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

type LocalPublishersRef<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

pub struct LocalPublishers<'a, Item, Err>(LocalPublishersRef<'a, Item, Err>);

pub type LocalSubject<'a, Item, Err> =
  Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>;

impl<'a, Item, Err> Clone for LocalPublishers<'a, Item, Err> {
  fn clone(&self) -> Self { LocalPublishers(self.0.clone()) }
}

type SharedPublishersRef<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub type SharedSubject<Item, Err> =
  Subject<SharedPublishers<Item, Err>, SharedSubscription>;

pub struct SharedPublishers<Item, Err>(SharedPublishersRef<Item, Err>);

impl<Item, Err> Clone for SharedPublishers<Item, Err> {
  fn clone(&self) -> Self { SharedPublishers(self.0.clone()) }
}

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  pub fn local() -> Self {
    Subject {
      observers: LocalPublishers(Rc::new(RefCell::new(vec![]))),
      subscription: LocalSubscription::default(),
    }
  }
}

impl<Item, Err> SharedSubject<Item, Err> {
  pub fn shared() -> Self {
    Subject {
      observers: SharedPublishers(Arc::new(Mutex::new(vec![]))),
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

impl<'a, Item, Err, U> IntoShared for Subject<LocalPublishers<'a, Item, Err>, U>
where
  U: IntoShared,
  Item: 'static,
  Err: 'static,
{
  type Shared = Subject<SharedPublishers<Item, Err>, U::Shared>;
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
      SharedPublishers(Arc::new(Mutex::new(vec![])))
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

impl<'a, Item, Err, O>
  RawSubscribable<Item, Err, Subscriber<O, LocalSubscription>>
  for LocalSubject<'a, Item, Err>
where
  O: Observer<Item, Err> + 'a,
{
  type Unsub = LocalSubscription;
  fn raw_subscribe(
    mut self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.0.borrow_mut().push(Box::new(subscriber));
    subscription
  }
}

impl<Item, Err, O, S> RawSubscribable<Item, Err, Subscriber<O, S>>
  for SharedSubject<Item, Err>
where
  S: IntoShared<Shared = SharedSubscription> + Clone,
  O: IntoShared,
  O::Shared: Observer<Item, Err>,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(mut self, subscriber: Subscriber<O, S>) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let subscription = subscriber.subscription.clone();
    self.subscription.add(subscription.clone());
    self.observers.0.lock().unwrap().push(Box::new(subscriber));
    subscription
  }
}

impl<O, S> Fork for Subject<O, S>
where
  Self: Clone,
{
  type Output = Self;
  fn fork(&self) -> Self::Output { self.clone() }
}

impl<'a, Item, Err, S> Observer<Item, Err>
  for Subject<LocalPublishers<'a, Item, Err>, S>
where
  S: SubscriptionLike,
{
  fn next(&mut self, value: &Item) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.borrow_mut();
      publishers.drain_filter(|subscriber| {
        subscriber.next(&value);
        subscriber.is_closed()
      });
    }
  }
  fn error(&mut self, err: &Err) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.borrow_mut();
      publishers.iter_mut().for_each(|subscriber| {
        subscriber.error(err);
      });
      publishers.clear();
      self.subscription.unsubscribe();
    };
  }
  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.borrow_mut();
      publishers.iter_mut().for_each(|subscriber| {
        subscriber.complete();
      });
      publishers.clear();
      self.subscription.unsubscribe();
    }
  }
}

impl<Item, Err, S> Observer<Item, Err>
  for Subject<SharedPublishers<Item, Err>, S>
where
  S: SubscriptionLike,
{
  fn next(&mut self, value: &Item) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.lock().unwrap();
      publishers.drain_filter(|subscriber| {
        subscriber.next(&value);
        subscriber.is_closed()
      });
    }
  }
  fn error(&mut self, err: &Err) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.lock().unwrap();
      publishers.iter_mut().for_each(|subscriber| {
        subscriber.error(err);
      });
      publishers.clear();
      self.subscription.unsubscribe();
    };
  }
  fn complete(&mut self) {
    if !self.subscription.is_closed() {
      let mut publishers = self.observers.0.lock().unwrap();
      publishers.iter_mut().for_each(|subscriber| {
        subscriber.complete();
      });
      publishers.clear();
      self.subscription.unsubscribe();
    }
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::local();
      broadcast.fork().subscribe(|v: &i32| i = *v * 2);
      broadcast.next(&1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::local();
    broadcast
      .fork()
      .subscribe_err(|_: &i32| {}, |e: &_| panic!(*e));
    broadcast.next(&1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = Subject::local();
      subject.fork().subscribe(|v| i = *v).unsubscribe();
      subject.next(&100);
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
      move |v: &i32| {
        *value.lock().unwrap() = *v;
      },
    );

    subject.next(&100);
    std::thread::sleep(std::time::Duration::from_micros(1));

    assert_eq!(*c_v.lock().unwrap(), 100);
  }
}
