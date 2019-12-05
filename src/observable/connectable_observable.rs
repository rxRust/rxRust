use crate::prelude::*;
use crate::subject::{LocalSubject, SharedSubject};

pub struct LocalConnectableObservable<'a, O, Item, Err> {
  source: O,
  subject: LocalSubject<'a, Item, Err>,
}

pub struct SharedConnectableObservable<O, Item, Err> {
  source: O,
  subject: SharedSubject<Item, Err>,
}

pub trait Connect {
  type Unsub;
  fn connect(self) -> Self::Unsub;
}

impl<'a, O, Item, Err, SO, SU> RawSubscribable<Item, Err, Subscriber<SO, SU>>
  for LocalConnectableObservable<'a, O, Item, Err>
where
  SO: Observer<Item, Err> + 'a,
  SU: SubscriptionLike + Clone + 'static,
{
  type Unsub = SU;

  #[inline(always)]
  fn raw_subscribe(self, subscriber: Subscriber<SO, SU>) -> Self::Unsub {
    self.subject.raw_subscribe(subscriber)
  }
}

impl<Item, Err, SO, O, U> RawSubscribable<Item, Err, Subscriber<SO, U>>
  for SharedConnectableObservable<O, Item, Err>
where
  SO: IntoShared,
  SO::Shared: Observer<Item, Err>,
  U: IntoShared,
  U::Shared: SubscriptionLike + Clone + 'static,
{
  type Unsub = U::Shared;

  #[inline(always)]
  fn raw_subscribe(self, subscriber: Subscriber<SO, U>) -> Self::Unsub {
    self.subject.raw_subscribe(subscriber)
  }
}

impl<'a, Item, Err, O> LocalConnectableObservable<'a, O, Item, Err> {
  pub fn local(observable: O) -> Self {
    Self {
      source: observable,
      subject: Subject::local(),
    }
  }
}

impl<'a, O, Item, Err> Connect for LocalConnectableObservable<'a, O, Item, Err>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<LocalSubject<'a, Item, Err>, LocalSubscription>,
  >,
{
  type Unsub = O::Unsub;
  fn connect(self) -> Self::Unsub {
    self.source.raw_subscribe(Subscriber {
      observer: self.subject.fork(),
      subscription: self.subject.subscription,
    })
  }
}

impl<O, Item, Err> SharedConnectableObservable<O, Item, Err>
where
  O: IntoShared,
{
  pub fn shared(
    observable: O,
  ) -> SharedConnectableObservable<O::Shared, Item, Err> {
    SharedConnectableObservable {
      source: observable.to_shared(),
      subject: Subject::shared(),
    }
  }
}

impl<O, Item, Err> Connect for SharedConnectableObservable<O, Item, Err>
where
  O: RawSubscribable<
    Item,
    Err,
    Subscriber<SharedSubject<Item, Err>, SharedSubscription>,
  >,
{
  type Unsub = O::Unsub;
  fn connect(self) -> Self::Unsub {
    self.source.raw_subscribe(Subscriber {
      observer: self.subject.fork(),
      subscription: self.subject.subscription,
    })
  }
}

impl<'a, Item, Err, O> IntoShared
  for LocalConnectableObservable<'a, O, Item, Err>
where
  O: IntoShared,
  Item: 'static,
  Err: 'static,
{
  type Shared = SharedConnectableObservable<O::Shared, Item, Err>;
  fn to_shared(self) -> Self::Shared {
    SharedConnectableObservable {
      source: self.source.to_shared(),
      subject: self.subject.to_shared(),
    }
  }
}

impl<Item, Err, O> IntoShared for SharedConnectableObservable<O, Item, Err>
where
  O: IntoShared,
  Item: 'static,
  Err: 'static,
{
  type Shared = SharedConnectableObservable<O::Shared, Item, Err>;
  fn to_shared(self) -> Self::Shared {
    SharedConnectableObservable {
      source: self.source.to_shared(),
      subject: self.subject,
    }
  }
}

impl<O, Item, Err> Fork for SharedConnectableObservable<O, Item, Err> {
  type Output = SharedSubject<Item, Err>;
  #[inline(always)]
  fn fork(&self) -> Self::Output { self.subject.fork() }
}

impl<'a, O, Item, Err> Fork for LocalConnectableObservable<'a, O, Item, Err> {
  type Output = LocalSubject<'a, Item, Err>;
  #[inline(always)]
  fn fork(&self) -> Self::Output { self.subject.fork() }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of(100);
    let connected = LocalConnectableObservable::local(o);
    let mut first = 0;
    let mut second = 0;
    connected.fork().subscribe(|v| first = v);
    connected.fork().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn fork_and_shared() {
    let o = observable::of(100);
    let connected = LocalConnectableObservable::local(o).to_shared();
    connected.fork().subscribe(|_| {});
    connected.fork().subscribe(|_| {});

    connected.connect();
  }
}
