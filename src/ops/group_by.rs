use crate::prelude::*;

use crate::impl_local_shared_both;
use std::{clone::Clone, cmp::Eq, collections::HashMap, hash::Hash};

/// Observable used to keep track of the key of the items emitted by the contained subject.
#[derive(Clone)]
pub struct KeyObservable<Key, Sub> {
  pub key: Key,
  subject: Sub,
}

impl<Key, Sub> Observable for KeyObservable<Key, Sub>
where
  Sub: Observable + Observer,
  Key: Hash + Clone + Eq,
{
  type Item = <Sub as Observable>::Item;
  type Err = <Sub as Observable>::Err;
}

impl_local_shared_both! {
  impl<Key, Sub> KeyObservable<Key, Sub>;
  type Unsub = Sub::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self
    .subject
    .actual_subscribe($observer)
  }
  where
    Sub: Observer + @ctx::Observable,
    Key: Hash + Clone + Eq,
}

#[derive(Clone)]
pub struct GroupByObserver<Obs, Discr, Key, Sub> {
  observer: Obs,
  discr: Discr,
  subjects: HashMap<Key, Sub>,
}

macro_rules! impl_observer {
  ($subject: ty) => {
    impl<'a, Obs, Discr, Key, Item, Err> Observer
      for GroupByObserver<Obs, Discr, Key, $subject>
    where
      Item: Clone,
      Err: Clone,
      Obs: Observer<Item = KeyObservable<Key, $subject>, Err = Err>,
      Discr: FnMut(&Item) -> Key + Clone,
      Key: Hash + Clone + Eq,
    {
      type Item = Item;
      type Err = Err;
      fn next(&mut self, value: Item) {
        let key = (self.discr)(&value);
        let subject = self.subjects.entry(key.clone()).or_insert_with(|| {
          let subject = <$subject>::new();
          let wrapper = KeyObservable {
            key,
            subject: subject.clone(),
          };
          self.observer.next(wrapper);
          subject
        });
        let _ = subject.next(value);
      }

      fn error(&mut self, err: Self::Err) {
        self.observer.error(err)
      }

      fn complete(&mut self) {
        self.observer.complete()
      }
    }
  };
}

impl_observer!(LocalSubject<'a, Item, Err>);
impl_observer!(SharedSubject<Item, Err>);
///////////////////////////////////////////////////////////////////////////////

/// Main observable returned by the group_by method.
/// `subject` is actually only present to keep track of the type of scheduling to use for
/// the emitted observables
#[derive(Clone)]
pub struct GroupByOp<Source, Discr, Subj> {
  pub(crate) source: Source,
  pub(crate) discr: Discr,
  pub(crate) _subject: TypeHint<Subj>,
}

impl<Source, Discr, Key, Subj> Observable for GroupByOp<Source, Discr, Subj>
where
  Source: Observable,
  Discr: FnMut(&Source::Item) -> Key,
  Key: Hash + Eq,
  Subj: Observable,
{
  type Item = KeyObservable<Key, Subj>;
  type Err = Source::Err;
}

impl<'a, Source, Discr, Key> LocalObservable<'a>
  for GroupByOp<Source, Discr, LocalSubject<'a, Source::Item, Source::Err>>
where
  Source: LocalObservable<'a> + 'a,
  Source::Item: 'a + Clone,
  Source::Err: Clone,
  Discr: FnMut(&Source::Item) -> Key + 'a + Clone,
  Key: 'a + Hash + Clone + Eq,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.source.actual_subscribe(GroupByObserver {
      observer,
      discr: self.discr,
      subjects: HashMap::<Key, LocalSubject<Source::Item, Source::Err>>::new(),
    })
  }
}

impl<Source, Discr, Key> SharedObservable
  for GroupByOp<Source, Discr, SharedSubject<Source::Item, Source::Err>>
where
  Source: SharedObservable + Send + Sync + 'static,
  Source::Item: Clone + Send + Sync,
  Source::Err: Clone + Send + Sync,
  Discr: FnMut(&Source::Item) -> Key + Clone + Send + Sync + 'static,
  Key: Hash + Clone + Eq + Send + Sync + 'static,
{
  type Unsub = Source::Unsub;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    self.source.actual_subscribe(GroupByObserver {
      observer,
      discr: self.discr,
      subjects: HashMap::<Key, SharedSubject<Source::Item, Source::Err>>::new(),
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

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn group_by_shared() {
    let s = observable::of(0).group_by(|_| "zero");
    s.into_shared().subscribe(|_| {});
  }

  #[test]
  fn it_only_subscribes_once_local() {
    let obs_count = MutRc::own(0);
    observable::create(|subscriber| {
      subscriber.next(1);
      subscriber.next(2);
      subscriber.complete();
    })
    .group_by(|value: &i64| *value)
    .subscribe(|group| {
      let obs_clone = obs_count.clone();
      group.subscribe(move |_| {
        *obs_clone.rc_deref_mut() += 1;
      });
    });
    assert_eq!(2, *obs_count.rc_deref());
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn it_only_subscribes_once_shared() {
    let value = MutArc::own(0);
    let v_c = value.clone();
    observable::create(|subscriber| {
      subscriber.next(1);
      subscriber.next(2);
      subscriber.complete();
    })
    .group_by(move |value: &i64| *value)
    .into_shared()
    .subscribe(move |group| {
      let v_c_c = v_c.clone();
      group.into_shared().subscribe(move |_| {
        *v_c_c.rc_deref_mut() += 1;
      });
    });
    assert_eq!(2, *value.rc_deref());
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_group_by);

  fn bench_group_by(b: &mut bencher::Bencher) {
    b.iter(group_by_parity);
  }
}
