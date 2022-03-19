use crate::{
  impl_local_shared_both,
  prelude::*,
  subject::{LocalSubject, SharedSubject},
};
use ops::ref_count::{InnerRefCount, RefCount};

pub struct ConnectableObservable<Src, Sbj> {
  source: Src,
  subject: Sbj,
}

impl<Src, Sbj> ConnectableObservable<Src, Sbj> {}

impl<Src, Sbj> Observable for ConnectableObservable<Src, Sbj>
where
  Sbj: Observable,
{
  type Item = Sbj::Item;
  type Err = Sbj::Err;
}

impl_local_shared_both! {
  impl<Src, Sbj> ConnectableObservable<Src, Sbj>;
  type Unsub = Sbj::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self.subject.actual_subscribe($observer)
  }
  where
    Sbj: @ctx::Observable
}

impl<Src, Sbj> ConnectableObservable<Src, Sbj> {
  #[inline]
  pub fn new(source: Src) -> Self
  where
    Sbj: Default,
  {
    ConnectableObservable {
      source,
      subject: <_>::default(),
    }
  }

  #[inline]
  pub fn fork(&self) -> Sbj
  where
    Sbj: Clone,
  {
    self.subject.clone()
  }
}

impl<'a, Src, Item, Err>
  ConnectableObservable<Src, LocalSubject<'a, Item, Err>>
{
  #[inline]
  pub fn local(source: Src) -> Self {
    ConnectableObservable {
      source,
      subject: <_>::default(),
    }
  }
}

impl<Src, Item, Err> ConnectableObservable<Src, SharedSubject<Item, Err>> {
  #[inline]
  pub fn shared(source: Src) -> Self {
    ConnectableObservable {
      source,
      subject: <_>::default(),
    }
  }
}

pub trait Connect {
  type R;
  type Unsub;
  fn into_ref_count(self) -> Self::R;
  fn connect(self) -> Self::Unsub;
}

type LocalInnerRefCount<'a, Src> = MutRc<
  InnerRefCount<
    Src,
    LocalSubject<'a, <Src as Observable>::Item, <Src as Observable>::Err>,
    <Src as LocalObservable<'a>>::Unsub,
  >,
>;
impl<'a, Src> Connect
  for ConnectableObservable<Src, LocalSubject<'a, Src::Item, Src::Err>>
where
  Src: LocalObservable<'a>,
  Src::Item: Clone + 'a,
  Src::Err: Clone + 'a,
{
  type R = RefCount<LocalInnerRefCount<'a, Src>>;
  type Unsub = Src::Unsub;

  #[inline]
  fn into_ref_count(self) -> Self::R { RefCount::local(self) }

  #[inline]
  fn connect(self) -> Self::Unsub { self.source.actual_subscribe(self.subject) }
}

type SharedInnerRefCount<Src> = MutArc<
  InnerRefCount<
    Src,
    SharedSubject<<Src as Observable>::Item, <Src as Observable>::Err>,
    <Src as SharedObservable>::Unsub,
  >,
>;

impl<Src> Connect
  for ConnectableObservable<Src, SharedSubject<Src::Item, Src::Err>>
where
  Src: SharedObservable,
  Src::Item: Clone + Send + Sync + 'static,
  Src::Err: Clone + Send + Sync + 'static,
{
  type R = RefCount<SharedInnerRefCount<Src>>;
  type Unsub = Src::Unsub;

  #[inline]
  fn into_ref_count(self) -> Self::R { RefCount::shared(self) }

  #[inline]
  fn connect(self) -> Self::Unsub { self.source.actual_subscribe(self.subject) }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of(100);
    let connected = ConnectableObservable::local(o);
    let mut first = 0;
    let mut second = 0;
    connected.fork().subscribe(|v| first = v);
    connected.fork().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn fork_and_shared() {
    let o = observable::of(100);
    let connected = ConnectableObservable::shared(o);
    connected.fork().into_shared().subscribe(|_| {});
    connected.fork().into_shared().subscribe(|_| {});

    connected.connect();
  }
  #[test]
  fn publish_smoke() {
    let p = observable::of(100).publish::<LocalSubject<'_, _, _>>();
    let mut first = 0;
    let mut second = 0;
    p.fork().subscribe(|v| first = v);
    p.fork().subscribe(|v| second = v);

    p.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_connectable);

  fn bench_connectable(b: &mut bencher::Bencher) { b.iter(smoke); }
}
