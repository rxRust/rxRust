use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};
use std::collections::VecDeque;

/// An Observable that combines from two other two Observables.
///
/// This struct is created by the zip method on [Observable](Observable::zip).
/// See its documentation for more.
#[derive(Clone)]
pub struct ZipOp<A, B> {
  a: A,
  b: B,
}

#[derive(Clone)]
pub struct ZipOpThreads<A, B> {
  a: A,
  b: B,
}

macro_rules! impl_zip_op {
  ($name:ident, $rc: ident) => {
    impl<A, B> $name<A, B> {
      pub fn new(a: A, b: B) -> Self {
        Self { a, b }
      }
    }

    impl<A, B, ItemA, ItemB, Err, O> Observable<(ItemA, ItemB), Err, O>
      for $name<A, B>
    where
      O: Observer<(ItemA, ItemB), Err>,
      A: Observable<
        ItemA,
        Err,
        AObserver<$rc<ZipObserver<O, ItemA, ItemB>>, ItemB>,
      >,
      B: Observable<
        ItemB,
        Err,
        BObserver<$rc<ZipObserver<O, ItemA, ItemB>>, ItemA>,
      >,
    {
      type Unsub = ZipSubscription<A::Unsub, B::Unsub>;
      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let o_zip = ZipObserver::new(observer);
        let o_zip = $rc::own(o_zip);
        let a_unsub = self
          .a
          .actual_subscribe(AObserver(o_zip.clone(), TypeHint::new()));
        let b_unsub =
          self.b.actual_subscribe(BObserver(o_zip, TypeHint::new()));

        ZipSubscription::new(a_unsub, b_unsub)
      }
    }

    impl<A, B, ItemA, ItemB, Err> ObservableExt<(ItemA, ItemB), Err>
      for $name<A, B>
    where
      A: ObservableExt<ItemA, Err>,
      B: ObservableExt<ItemB, Err>,
    {
    }

    impl<O, ItemA, ItemB, Err> Observer<ZipItem<ItemA, ItemB>, Err>
      for $rc<ZipObserver<O, ItemA, ItemB>>
    where
      O: Observer<(ItemA, ItemB), Err>,
    {
      fn next(&mut self, value: ZipItem<ItemA, ItemB>) {
        let mut inner = self.rc_deref_mut();
        let mut zip_value = None;
        match value {
          ZipItem::ItemA(v) => {
            if !inner.b.is_empty() {
              zip_value = Some((v, inner.b.pop_front().unwrap()));
            } else {
              inner.a.push_back(v);
            }
          }
          ZipItem::ItemB(v) => {
            if !inner.a.is_empty() {
              zip_value = Some((inner.a.pop_front().unwrap(), v));
            } else {
              inner.b.push_back(v)
            }
          }
        }
        if let (Some(v), Some(observer)) = (zip_value, inner.observer.as_mut())
        {
          observer.next(v)
        }
      }

      fn error(self, err: Err) {
        if let Some(observer) = self.rc_deref_mut().observer.take() {
          observer.error(err);
        }
      }

      fn complete(self) {
        let mut inner = self.rc_deref_mut();
        if inner.completed_one {
          if let Some(observer) = inner.observer.take() {
            observer.complete();
          }
        } else {
          inner.completed_one = true;
        }
      }

      #[inline]
      fn is_finished(&self) -> bool {
        self
          .rc_deref()
          .observer
          .as_ref()
          .map_or(true, |o| o.is_finished())
      }
    }
  };
}

impl_zip_op!(ZipOp, MutRc);
impl_zip_op!(ZipOpThreads, MutArc);

enum ZipItem<A, B> {
  ItemA(A),
  ItemB(B),
}

pub struct ZipObserver<O, ItemA, ItemB> {
  observer: Option<O>,
  a: VecDeque<ItemA>,
  b: VecDeque<ItemB>,
  completed_one: bool,
}

impl<O, ItemA, ItemB> ZipObserver<O, ItemA, ItemB> {
  fn new(o: O) -> Self {
    ZipObserver {
      observer: Some(o),
      a: VecDeque::default(),
      b: VecDeque::default(),
      completed_one: false,
    }
  }
}

pub struct AObserver<O, ItemB>(O, TypeHint<ItemB>);

impl<O, ItemA, ItemB, Err> Observer<ItemA, Err> for AObserver<O, ItemB>
where
  O: Observer<ZipItem<ItemA, ItemB>, Err>,
{
  #[inline]
  fn next(&mut self, value: ItemA) {
    self.0.next(ZipItem::ItemA(value));
  }

  #[inline]
  fn error(self, err: Err) {
    self.0.error(err)
  }

  #[inline]
  fn complete(self) {
    self.0.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.0.is_finished()
  }
}

pub struct BObserver<O, A>(O, TypeHint<A>);

impl<O, ItemA, ItemB, Err> Observer<ItemB, Err> for BObserver<O, ItemA>
where
  O: Observer<ZipItem<ItemA, ItemB>, Err>,
{
  #[inline]
  fn next(&mut self, value: ItemB) {
    self.0.next(ZipItem::ItemB(value));
  }

  #[inline]
  fn error(self, err: Err) {
    self.0.error(err)
  }

  #[inline]
  fn complete(self) {
    self.0.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.0.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[test]
  fn smoke() {
    let zip = observable::from_iter(0..10).zip(observable::from_iter(0..10));
    let zipped_count = Arc::new(AtomicUsize::new(0));
    zip
      .clone()
      .count()
      .subscribe(|v| zipped_count.store(v, Ordering::Relaxed));
    let mut zipped_sum = 0;
    assert_eq!(zipped_count.load(Ordering::Relaxed), 10);
    zip.map(|(a, b)| a + b).sum().subscribe(|v| zipped_sum = v);
    assert_eq!(zipped_sum, 90);
  }

  #[test]
  fn complete() {
    let mut complete = false;
    {
      let s1 = Subject::default();
      s1.clone()
        .zip(Subject::default())
        .on_complete(|| complete = true)
        .subscribe(|_: ((), ())| {});

      s1.complete();
    }
    assert!(!complete);

    {
      let s1 = Subject::default();
      let s2 = Subject::default();
      s1.clone()
        .zip(s2.clone())
        .on_complete(|| complete = true)
        .subscribe(|_: ((), ())| {});

      s1.complete();
      s2.complete();
    }
    assert!(complete);
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
