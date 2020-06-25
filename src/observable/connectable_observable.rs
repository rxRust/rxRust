use crate::prelude::*;
use crate::subject::{LocalSubject, SharedSubject};
use observable::observable_proxy_impl;
use ops::ref_count::{RefCount, RefCountCreator};

#[derive(Clone, Default)]
pub struct ConnectableObservable<Source, Subject> {
  pub(crate) source: Source,
  pub(crate) subject: Subject,
}

impl<Source, Subject: Default> ConnectableObservable<Source, Subject> {
  pub fn new(source: Source) -> Self {
    ConnectableObservable {
      source,
      subject: Subject::default(),
    }
  }
}
observable_proxy_impl!(ConnectableObservable, Source, Subject);

pub type LocalConnectableObservable<'a, S, Item, Err> =
  ConnectableObservable<S, LocalSubject<'a, Item, Err>>;

pub type SharedConnectableObservable<S, Item, Err> =
  ConnectableObservable<S, SharedSubject<Item, Err>>;

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  type Unsub = $subscription;
  #[inline(always)]
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.subject.actual_subscribe(subscriber)
  }
}

impl<'a, S, Item, Err> LocalObservable<'a>
  for LocalConnectableObservable<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
{
  observable_impl!(LocalSubscription, 'a);
}

impl<S, Item, Err> SharedObservable
  for SharedConnectableObservable<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
  S: SharedObservable<Item = Item, Err = Err>,
{
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

impl<Source, Subject> ConnectableObservable<Source, Subject>
where
  Source: Clone,
{
  #[inline]
  pub fn ref_count<Inner: RefCountCreator<Connectable = Self>>(
    self,
  ) -> RefCount<Inner, Self> {
    Inner::new(self)
  }
}

impl<'a, S, Item, Err> LocalConnectableObservable<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
  Item: Clone + 'a,
  Err: Clone + 'a,
{
  pub fn connect(self) -> S::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: self.subject.observers,
      subscription: self.subject.subscription,
    })
  }
}

impl<S, Item, Err> SharedConnectableObservable<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
  Item: Clone + Send + Sync + 'static,
  Err: Clone + Send + Sync + 'static,
{
  pub fn connect(self) -> S::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: self.subject.observers,
      subscription: self.subject.subscription,
    })
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of(100);
    let connected = ConnectableObservable::new(o);
    let mut first = 0;
    let mut second = 0;
    connected.clone().subscribe(|v| first = v);
    connected.clone().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn fork_and_shared() {
    let o = observable::of(100);
    let connected = ConnectableObservable::new(o);
    connected.clone().to_shared().subscribe(|_| {});
    connected.clone().to_shared().subscribe(|_| {});

    connected.connect();
  }
}

#[test]
fn publish_smoke() {
  let p = observable::of(100).publish();
  let mut first = 0;
  let mut second = 0;
  p.clone().subscribe(|v| first = v);
  p.clone().subscribe(|v| second = v);

  p.connect();
  assert_eq!(first, 100);
  assert_eq!(second, 100);
}

#[test]
fn filter() {
  use crate::ops::FilterMap;
  use crate::subject::MutRefValue;
  let mut subject = Subject::new();

  unsafe {
    subject
      .clone()
      .mut_ref_all()
      .filter_map::<fn(&mut i32) -> Option<&mut i32>, _, _>(|v| Some(v))
      .publish()
      .subscribe_err(|_: &mut _| {}, |_: &mut i32| {});
  }

  subject.next(MutRefValue(&mut 1));
  subject.error(MutRefValue(&mut 2));
}
