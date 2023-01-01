use crate::prelude::*;
use std::{clone::Clone, collections::HashMap, hash::Hash};

/// Observable used to keep track of the key of the items emitted by the contained subject.
#[derive(Clone)]
pub struct KeyObservable<Key, Subject> {
  pub key: Key,
  subject: Subject,
}

impl<Key, Item, Err, O, Subject> Observable<Item, Err, O>
  for KeyObservable<Key, Subject>
where
  Subject: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  type Unsub = Subject::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.subject.actual_subscribe(observer)
  }
}

impl<Item, Err, Key, Subject> ObservableExt<Item, Err>
  for KeyObservable<Key, Subject>
where
  Subject: Observer<Item, Err>,
{
}

#[derive(Clone)]
pub struct GroupByObserver<O, Discr, Key, Subject> {
  observer: O,
  discr: Discr,
  subjects: HashMap<Key, Subject>,
}

///////////////////////////////////////////////////////////////////////////////

/// Main observable returned by the group_by method.
/// `subject` is actually only present to keep track of the type of scheduling to use for
/// the emitted observables
#[derive(Clone)]
pub struct GroupByOp<Source, Discr, Subject> {
  pub(crate) source: Source,
  pub(crate) discr: Discr,
  _hint: TypeHint<Subject>,
}

impl<Source, Discr, Subject> GroupByOp<Source, Discr, Subject> {
  #[inline]
  pub fn new(source: Source, discr: Discr) -> Self {
    Self {
      source,
      discr,
      _hint: TypeHint::default(),
    }
  }
}

macro_rules! impl_observable_for_group_by {
  ($ty: ty $(,$lf:lifetime)?) => {
    impl<$($lf,)? Source, Discr, Key, Item, Err, O>
      Observable<KeyObservable<Key, $ty>, Err, O>
      for GroupByOp<Source, Discr, $ty>
    where
      O: Observer<KeyObservable<Key, $ty>, Err>,
      Source: Observable<Item, Err, GroupByObserver<O, Discr, Key, $ty>>,
      Discr: FnMut(&Item) -> Key,
      Key: Hash + Eq + Clone,
      Item: Clone,
      Err: Clone,
    {
      type Unsub = Source::Unsub;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        self.source.actual_subscribe(GroupByObserver {
          observer,
          discr: self.discr,
          subjects: <_>::default(),
        })
      }
    }

    impl<$($lf,)? Source, Discr, Key, Item, Err>
      ObservableExt<KeyObservable<Key, $ty>, Err>
      for GroupByOp<Source, Discr, $ty>
    where
      Source: ObservableExt<Item, Err>
    {
    }
  };
}

impl_observable_for_group_by!(Subject<'a, Item, Err>, 'a);
impl_observable_for_group_by!(SubjectThreads<Item, Err>);

impl<Discr, Key, Subject, Item, Err, O> Observer<Item, Err>
  for GroupByObserver<O, Discr, Key, Subject>
where
  O: Observer<KeyObservable<Key, Subject>, Err>,
  Discr: FnMut(&Item) -> Key,
  Key: Hash + Eq + Clone,
  Subject: Clone + Default + Observer<Item, Err>,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    let key = (self.discr)(&value);
    let subject = self.subjects.entry(key.clone()).or_insert_with(|| {
      let subject = Subject::default();
      let wrapper = KeyObservable { key, subject: subject.clone() };
      self.observer.next(wrapper);
      subject
    });
    subject.next(value);
  }

  #[inline]
  fn error(mut self, err: Err) {
    for (_, subject) in self.subjects.drain() {
      subject.error(err.clone());
    }
    self.observer.error(err)
  }

  #[inline]
  fn complete(mut self) {
    for (_, subject) in self.subjects.drain() {
      subject.complete();
    }
    self.observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use crate::rc::{MutRc, RcDeref, RcDerefMut};

  #[test]
  fn group_by_parity() {
    let mut obs_count = 0;
    observable::from_iter(0..100)
      .group_by::<_, _, Subject<_, _>>(|val| val % 2 == 0)
      .subscribe(|obs| {
        obs_count += 1;
        if obs.key {
          let _ = obs.subscribe(|val| assert_eq!(val % 2, 0));
        } else {
          let _ = obs.subscribe(|val| assert_ne!(val % 2, 0));
        }
      });
    assert_eq!(obs_count, 2);
  }

  #[test]
  fn it_only_subscribes_once_local() {
    let obs_count = MutRc::own(0);
    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(1);
      subscriber.next(2);
      subscriber.complete();
    })
    .group_by::<_, _, Subject<_, _>>(|value: &i64| *value)
    .subscribe(|group| {
      let obs_clone = obs_count.clone();
      group.subscribe(move |_| {
        *obs_clone.rc_deref_mut() += 1;
      });
    });
    assert_eq!(2, *obs_count.rc_deref());
  }

  #[test]
  fn propagates_complete() {
    let mut sum = 0;
    from_iter(vec![1, 2, 3])
      .group_by::<_, _, Subject<_, _>>(|v| *v)
      .flat_map(|group| group.sum())
      .subscribe(|avg| {
        sum += avg;
      });
    assert_eq!(sum, 6);
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
