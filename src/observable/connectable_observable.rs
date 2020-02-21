use crate::prelude::*;
use crate::subject::{LocalSubject, SharedSubject};
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

pub type LocalConnectableObservable<'a, S, Item, Err> =
  ConnectableObservable<S, LocalSubject<'a, Item, Err>>;

pub type SharedConnectableObservable<S, Item, Err> =
  ConnectableObservable<S, SharedSubject<Item, Err>>;

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

impl<'a, S, Item, Err> Observable<'a>
  for LocalConnectableObservable<'a, S, Item, Err>
where
  S: Observable<'a, Item = Item, Err = Err>,
  S: Observable<'a, Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  observable_impl!(LocalSubscription, 'a);
}

impl<S, Item, Err> SharedObservable
  for SharedConnectableObservable<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
  S: SharedObservable<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}
impl<Source, Subject> ConnectableObservable<Source, Subject> {
  #[inline]
  pub fn ref_count<Inner: RefCountCreator<Connectable = Self>>(
    self,
  ) -> RefCount<Inner, Self> {
    Inner::new(self)
  }
}

impl<'a, S, Item, Err> LocalConnectableObservable<'a, S, Item, Err>
where
  S: Observable<'a, Item = Item, Err = Err>,
  Item: Copy + 'a,
  Err: Copy + 'a,
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
  Item: Copy + Send + Sync + 'static,
  Err: Copy + Send + Sync + 'static,
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
