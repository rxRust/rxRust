use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Subject<O, S> {
  observers: O,
  subscription: S,
}

type LocalPublishersRef<'a, Item, Err> =
  Rc<RefCell<Vec<Box<dyn Publisher<Item, Err> + 'a>>>>;

pub struct LocalPublishers<'a, Item, Err>(LocalPublishersRef<'a, Item, Err>);

impl<'a, Item, Err> Clone for LocalPublishers<'a, Item, Err> {
  fn clone(&self) -> Self { LocalPublishers(self.0.clone()) }
}

type SharedPublishersRef<Item, Err> =
  Arc<Mutex<Vec<Box<dyn Publisher<Item, Err> + Send + Sync>>>>;

pub struct SharedPublishers<Item, Err>(SharedPublishersRef<Item, Err>);

impl<Item, Err> Clone for SharedPublishers<Item, Err> {
  fn clone(&self) -> Self { SharedPublishers(self.0.clone()) }
}

impl<'a, Item, Err> Subject<LocalPublishers<'a, Item, Err>, LocalSubscription> {
  pub fn local() -> Self {
    Subject {
      observers: LocalPublishers(Rc::new(RefCell::new(vec![]))),
      subscription: LocalSubscription::default(),
    }
  }
}

impl<Item, Err> Subject<SharedPublishers<Item, Err>, SharedSubscription> {
  pub fn shared() -> Self {
    Subject {
      observers: SharedPublishers(Arc::new(Mutex::new(vec![]))),
      subscription: SharedSubscription::default(),
    }
  }
}
impl<Item, Err> IntoShared
  for Subject<SharedPublishers<Item, Err>, SharedSubscription>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<'a, Item, Err, O>
  RawSubscribable<Item, Err, Subscriber<O, LocalSubscription>>
  for Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>
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
  for Subject<SharedPublishers<Item, Err>, SharedSubscription>
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
}
