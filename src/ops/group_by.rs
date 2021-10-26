use crate::prelude::*;
use crate::{complete_proxy_impl, error_proxy_impl};

use std::{clone::Clone, cmp::Eq, collections::HashSet, hash::Hash};

/// Observer filtering out the data that does not match its key.
#[derive(Clone)]
struct GroupObserver<Obs, Discr, Key> {
  observer: Obs,
  discr: Discr,
  key: Key,
}

impl<Obs, Discr, Key, Item, Err> Observer for GroupObserver<Obs, Discr, Key>
where
  Obs: Observer<Item = Item, Err = Err>,
  Discr: FnMut(&Item) -> Key,
  Key: Hash + Eq,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if (self.discr)(&value) == self.key {
      self.observer.next(value);
    }
  }
  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
}

///////////////////////////////////////////////////////////////////////////////

/// Observable emited by the GroupByOp.
#[derive(Clone)]
pub struct GroupObservable<Source, Discr, Key> {
  source: Source,
  discr: Discr,
  key: Key,
}

impl<Source, Discr, Key> Observable for GroupObservable<Source, Discr, Key>
where
  Source: Observable,
{
  type Item = Source::Item;
  type Err = Source::Err;
}

impl<'a, Source, Discr, Key> LocalObservable<'a>
  for GroupObservable<Source, Discr, Key>
where
  Source: LocalObservable<'a>,
  Source::Item: 'a,
  Discr: FnMut(&Source::Item) -> Key + Clone + 'a,
  Key: Hash + Clone + Eq + 'a,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.source.actual_subscribe(Subscriber {
      observer: GroupObserver {
        observer: subscriber.observer,
        discr: self.discr,
        key: self.key,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<Source, Discr, Key> SharedObservable
  for GroupObservable<Source, Discr, Key>
where
  Source: SharedObservable,
  Source::Item: Send + Sync + 'static,
  Discr: FnMut(&Source::Item) -> Key + Clone + Send + Sync + 'static,
  Key: Hash + Clone + Eq + Send + Sync + 'static,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    self.source.actual_subscribe(Subscriber {
      observer: GroupObserver {
        observer: subscriber.observer,
        discr: self.discr,
        key: self.key,
      },
      subscription: subscriber.subscription,
    })
  }
}

///////////////////////////////////////////////////////////////////////////////

/// GroupByObserver keeps track of its source observable. Produces
/// GroupObservable objects for each key returned by the discriminator
/// function that was not yet encountered.
#[derive(Clone)]
pub struct GroupByObserver<Obs, Source, Discr, Key, Item> {
  observer: Obs,
  source: Source,
  discr: Discr,
  keys: HashSet<Key>,
  _marker: TypeHint<*const Item>,
}

impl<'a, Obs, Source, Discr, Key, Item, Err> Observer
  for GroupByObserver<Obs, Source, Discr, Key, Item>
where
  Obs: Observer<Item = GroupObservable<Source, Discr, Key>, Err = Err>,
  Source: Observable + Clone,
  Discr: FnMut(&Item) -> Key + Clone,
  Key: Hash + Clone + Eq,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    let key = (self.discr)(&value);
    if !self.keys.contains(&key) {
      let source = self.source.clone();
      let discr = self.discr.clone();
      self.keys.insert(key.clone());
      self.observer.next(GroupObservable { source, discr, key });
    };
  }
  error_proxy_impl!(Err, observer);
  complete_proxy_impl!(observer);
}

///////////////////////////////////////////////////////////////////////////////

/// Main observable returned by the group_by method.
#[derive(Clone)]
pub struct GroupByOp<Source, Discr> {
  pub(crate) source: Source,
  pub(crate) discr: Discr,
}

impl<Source, Discr, Key> Observable for GroupByOp<Source, Discr>
where
  Source: Observable,
  Discr: FnMut(&Source::Item) -> Key,
  Key: Hash + Eq,
{
  type Item = GroupObservable<Source, Discr, Key>;
  type Err = Source::Err;
}

impl<'a, Source, Discr, Key> LocalObservable<'a> for GroupByOp<Source, Discr>
where
  Source: LocalObservable<'a> + 'a + Clone,
  Source::Item: 'a,
  Discr: FnMut(&Source::Item) -> Key + 'a + Clone,
  Key: 'a + Hash + Clone + Eq,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    let source = self.source.clone();
    self.source.actual_subscribe(Subscriber {
      observer: GroupByObserver {
        observer: subscriber.observer,
        source,
        discr: self.discr,
        keys: HashSet::new(),
        _marker: TypeHint::new(),
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<'a, Source, Discr, Key> SharedObservable for GroupByOp<Source, Discr>
where
  Source: SharedObservable + Clone + Send + Sync + 'static,
  Source::Item: Send + Sync + 'static,
  Discr: FnMut(&Source::Item) -> Key + Clone + Send + Sync + 'static,
  Key: 'a + Hash + Clone + Eq + Send + Sync + 'static,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    let source = self.source.clone();
    self.source.actual_subscribe(Subscriber {
      observer: GroupByObserver {
        observer: subscriber.observer,
        source,
        discr: self.discr,
        keys: HashSet::new(),
        _marker: TypeHint::new(),
      },
      subscription: subscriber.subscription,
    })
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn group_by_parity() {
    let mut obs_count = 0;
    observable::from_iter(0..100)
      .group_by(|val| val % 2 == 0)
      .subscribe(|obs| {
        obs_count += 1;
        if obs.key {
          obs.subscribe(|val| assert_eq!(val % 2, 0));
        } else {
          obs.subscribe(|val| assert_ne!(val % 2, 0));
        }
      });
    assert_eq!(obs_count, 2);
  }

  #[test]
  fn group_by_shared() {
    let s = observable::of(0).group_by(|_| "zero");
    s.into_shared().subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_group_by);

  fn bench_group_by(b: &mut bencher::Bencher) { b.iter(group_by_parity); }
}
