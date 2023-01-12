use std::marker::PhantomData;

use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};

/// An Observable that combines from two other two Observables.
///
/// This struct is created by the with_latest_from method on
/// [Observable](Observable::with_latest_from). See its documentation for more.
#[derive(Clone)]
pub struct WithLatestFromOp<S, FS> {
  pub(crate) source: S,
  pub(crate) from: FS,
}

#[derive(Clone)]
pub struct WithLatestFromOpThreads<S, FS> {
  pub(crate) source: S,
  pub(crate) from: FS,
}

macro_rules! impl_with_last_from_op {
  ($name: ident, $rc: ident) => {
    impl<S, FS> $name<S, FS> {
      pub(crate) fn new(source: S, from: FS) -> Self {
        Self { source, from }
      }
    }

    impl<Source, From, O, ItemA, ItemB, Err> Observable<(ItemA, ItemB), Err, O>
      for $name<Source, From>
    where
      O: Observer<(ItemA, ItemB), Err>,
      Source:
        Observable<ItemA, Err, AObserver<$rc<Option<O>>, $rc<Option<ItemB>>>>,
      From: Observable<
        ItemB,
        Err,
        BObserver<$rc<Option<O>>, $rc<Option<ItemB>>, ItemA>,
      >,
      ItemB: Clone,
    {
      type Unsub = ZipSubscription<Source::Unsub, From::Unsub>;
      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let item = $rc::own(None);
        let source_observer = $rc::own(Some(observer));
        let from_observer = BObserver {
          observer: source_observer.clone(),
          value: item.clone(),
          _marker: std::marker::PhantomData::<ItemA>,
        };
        let from_unsub = self.from.actual_subscribe(from_observer);
        let source_unsub = self.source.actual_subscribe(AObserver {
          observer: source_observer,
          value: item,
        });

        ZipSubscription::new(source_unsub, from_unsub)
      }
    }

    impl<Source, From, ItemA, ItemB, Err> ObservableExt<(ItemA, ItemB), Err>
      for $name<Source, From>
    where
      Source: ObservableExt<ItemA, Err>,
      From: ObservableExt<ItemB, Err>,
    {
    }
  };
}

impl_with_last_from_op!(WithLatestFromOp, MutRc);
impl_with_last_from_op!(WithLatestFromOpThreads, MutArc);

#[derive(Clone)]
pub struct BObserver<O, V, ItemA> {
  observer: O,
  value: V,
  _marker: PhantomData<ItemA>,
}

impl<O, ItemA, ItemB, V, Err> Observer<ItemB, Err> for BObserver<O, V, ItemA>
where
  O: Observer<(ItemA, ItemB), Err>,
  V: RcDerefMut<Target = Option<ItemB>>,
{
  #[inline]
  fn next(&mut self, value: ItemB) {
    *self.value.rc_deref_mut() = Some(value);
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {}

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[derive(Clone)]
pub struct AObserver<O, V> {
  observer: O,
  value: V,
}

impl<ItemA, ItemB, Err, V, O> Observer<ItemA, Err> for AObserver<O, V>
where
  O: Observer<(ItemA, ItemB), Err>,
  ItemB: Clone,
  V: RcDeref<Target = Option<ItemB>>,
{
  fn next(&mut self, item: ItemA) {
    // should not write in one line early end value borrow.
    let v = self.value.rc_deref().clone();
    if let Some(v) = v {
      self.observer.next((item, v));
    }
  }

  #[inline]
  fn complete(self) {
    self.observer.complete();
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn simple() {
    let mut ret = String::new();

    {
      let mut s1 = Subject::default();
      let mut s2 = Subject::default();

      s1.clone().with_latest_from(s2.clone()).subscribe(|(a, b)| {
        ret.push(a);
        ret.push(b);
      });

      s1.next('1');
      s2.next('A');
      s1.next('2'); // 2A
      s2.next('B');
      s2.next('C');
      s2.next('D');
      s1.next('3'); // 3D
      s1.next('4'); // 4D
      s1.next('5'); // 5D
    }

    assert_eq!(ret, "2A3D4D5D");
  }

  #[test]
  fn smoke() {
    let mut a_store = vec![];
    let mut b_store = vec![];
    let mut numbers_store = vec![];

    {
      let mut numbers = Subject::default();
      let primary = numbers.clone().filter(|v| *v % 3 == 0);
      let secondary = numbers.clone().filter(|v| *v % 3 != 0);

      let with_latest_from =
        primary.clone().with_latest_from(secondary.clone());

      //  attach observers
      with_latest_from.subscribe(|v| numbers_store.push(v));
      primary.subscribe(|v| a_store.push(v));
      secondary.subscribe(|v| b_store.push(v));

      (0..10).for_each(|v| {
        numbers.next(v);
      });
    }

    assert_eq!(a_store, vec![0, 3, 6, 9]);
    assert_eq!(b_store, vec![1, 2, 4, 5, 7, 8]);
    assert_eq!(numbers_store, vec![(3, 2), (6, 5), (9, 8)]);
  }

  #[test]
  fn complete() {
    let mut complete = false;
    {
      let s1 = Subject::default();
      s1.clone()
        .with_latest_from(Subject::default())
        .on_complete(|| complete = true)
        .subscribe(|((), ())| {});

      s1.complete();
    }
    assert!(complete);

    complete = false;
    {
      let s1 = Subject::default();
      let s2 = Subject::default();
      s1.clone()
        .with_latest_from(s2.clone())
        .on_complete(|| complete = true)
        .subscribe(|((), ())| {});

      s2.complete();
    }
    assert!(!complete);
  }

  #[test]
  fn circular() {
    let mut subject_a = Subject::default();
    let mut subject_b = Subject::default();
    let mut cloned_subject_b = subject_b.clone();

    subject_a
      .clone()
      .with_latest_from(subject_b.clone())
      .subscribe(move |_| {
        cloned_subject_b.next(());
      });
    subject_b.next(());
    subject_a.next(());
    subject_a.next(());
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_zip);

  fn bench_zip(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }
}
