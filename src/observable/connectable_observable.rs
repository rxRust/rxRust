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
crate::observable_proxy_impl!(ConnectableObservable, Source, Subject);

pub type LocalConnectableObservable<'a, S, Item, Err> =
  ConnectableObservable<S, LocalSubject<'a, Item, Err>>;

pub type SharedConnectableObservable<S, Item, Err> =
  ConnectableObservable<S, SharedSubject<Item, Err>>;

impl<'a, S, Item, Err> LocalObservable<'a>
  for LocalConnectableObservable<'a, S, Item, Err>
where
  S: LocalObservable<'a, Item = Item, Err = Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.source.actual_subscribe(observer)
  }
}

impl<S, Item, Err> SharedObservable
  for SharedConnectableObservable<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self.source.actual_subscribe(observer)
  }
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
    self.source.actual_subscribe(self.subject)
  }
}

impl<S, Item, Err> SharedConnectableObservable<S, Item, Err>
where
  S: SharedObservable<Item = Item, Err = Err>,
  Item: Clone + Send + Sync + 'static,
  Err: Clone + Send + Sync + 'static,
{
  pub fn connect(self) -> S::Unsub {
    self.source.actual_subscribe(self.subject)
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
    connected.clone().into_shared().subscribe(|_| {});
    connected.clone().into_shared().subscribe(|_| {});

    connected.connect();
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
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_connectable);

  fn bench_connectable(b: &mut bencher::Bencher) { b.iter(smoke); }
}
