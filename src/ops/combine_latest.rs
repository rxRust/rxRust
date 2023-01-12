use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};

#[derive(Clone)]
pub struct CombineLatestOp<A, B, BinaryOp> {
  a: A,
  b: B,
  binary_op: BinaryOp,
}

#[derive(Clone)]
pub struct CombineLatestOpThread<A, B, BinaryOp> {
  a: A,
  b: B,
  binary_op: BinaryOp,
}

macro_rules! impl_combine_latest_op {
  ($name: ident, $rc: ident) => {
    impl<A, B, BinaryOp> $name<A, B, BinaryOp> {
      #[inline]
      pub(crate) fn new(
        a: A,
        b: B,
        binary_op: BinaryOp,
      ) -> $name<A, B, BinaryOp> {
        $name { a, b, binary_op }
      }
    }

    impl<A, B, ItemA, ItemB, Err, O, BinaryOp>
      Observable<(ItemA, ItemB), Err, O> for $name<A, B, BinaryOp>
    where
      O: Observer<(ItemA, ItemB), Err>,
      A: Observable<
        ItemA,
        Err,
        AObserver<$rc<CombineLatestObserver<O, ItemA, ItemB, BinaryOp>>, ItemB>,
      >,
      B: Observable<
        ItemB,
        Err,
        BObserver<$rc<CombineLatestObserver<O, ItemA, ItemB, BinaryOp>>, ItemA>,
      >,
      $rc<CombineLatestObserver<O, ItemA, ItemB, BinaryOp>>:
        Observer<CombineItem<ItemA, ItemB>, Err>,
    {
      type Unsub = ZipSubscription<A::Unsub, B::Unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let o_combine = CombineLatestObserver::new(observer, self.binary_op);
        let o_combine = $rc::own(o_combine);
        let a_unsub = self
          .a
          .actual_subscribe(AObserver(o_combine.clone(), TypeHint::new()));
        let b_unsub = self
          .b
          .actual_subscribe(BObserver(o_combine, TypeHint::new()));

        ZipSubscription::new(a_unsub, b_unsub)
      }
    }

    impl<A, B, ItemA, ItemB, Err, BinaryOp> ObservableExt<(ItemA, ItemB), Err>
      for $name<A, B, BinaryOp>
    where
      A: ObservableExt<ItemA, Err>,
      B: ObservableExt<ItemB, Err>,
    {
    }
  };
}

impl_combine_latest_op!(CombineLatestOp, MutRc);
impl_combine_latest_op!(CombineLatestOpThread, MutArc);

enum CombineItem<A, B> {
  ItemA(A),
  ItemB(B),
}

pub struct CombineLatestObserver<O, ItemA, ItemB, BinaryOp> {
  observer: Option<O>,
  a: Option<ItemA>,
  b: Option<ItemB>,
  binary_op: BinaryOp,
  completed_one: bool,
}

impl<O, A, B, BinaryOp> CombineLatestObserver<O, A, B, BinaryOp> {
  fn new(o: O, binary_op: BinaryOp) -> Self {
    CombineLatestObserver {
      observer: Some(o),
      a: None,
      b: None,
      binary_op,
      completed_one: false,
    }
  }
}

macro_rules! impl_combine_latest_observer {
  ($rc:ident) => {
    impl<O, A, B, OutputItem, BinaryOp, Err> Observer<CombineItem<A, B>, Err>
      for $rc<CombineLatestObserver<O, A, B, BinaryOp>>
    where
      O: Observer<OutputItem, Err>,
      BinaryOp: FnMut(A, B) -> OutputItem,
      A: Clone,
      B: Clone,
    {
      fn next(&mut self, value: CombineItem<A, B>) {
        let mut inner = self.rc_deref_mut();
        match value {
          CombineItem::ItemA(v) => {
            inner.a = Some(v);
          }
          CombineItem::ItemB(v) => {
            inner.b = Some(v);
          }
        }
        let CombineLatestObserver { observer, a, b, binary_op, .. } =
          &mut *inner;
        if let (Some(observer), Some(a), Some(b)) =
          (observer.as_mut(), a.clone(), b.clone())
        {
          observer.next(binary_op(a, b));
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

impl_combine_latest_observer!(MutRc);
impl_combine_latest_observer!(MutArc);
pub struct AObserver<O, B>(O, TypeHint<B>);

impl<O, A, B, Err> Observer<A, Err> for AObserver<O, B>
where
  O: Observer<CombineItem<A, B>, Err>,
{
  #[inline]
  fn next(&mut self, value: A) {
    self.0.next(CombineItem::ItemA(value));
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

impl<O, A, B, Err> Observer<B, Err> for BObserver<O, A>
where
  O: Observer<CombineItem<A, B>, Err>,
{
  fn next(&mut self, value: B) {
    self.0.next(CombineItem::ItemB(value));
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
mod tests {
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::time::Duration;

  use crate::observable::fake_timer::FakeClock;

  use super::*;

  #[test]
  fn combine_latest_base() {
    let clock = FakeClock::default();
    let x = Rc::new(RefCell::new(vec![]));

    let interval = clock.interval(Duration::from_millis(2));

    {
      let x_c = x.clone();
      interval
        .combine_latest(clock.interval(Duration::from_millis(3)), |a, b| (a, b))
        .take(7)
        .subscribe(move |v| {
          x_c.borrow_mut().push(v);
        });
      clock.advance(Duration::from_millis(50));
      {
        let v = x.borrow();
        assert_eq!(v.len(), 7);
        assert_eq!(
          v.as_ref(),
          vec![(0, 0), (1, 0), (2, 0), (2, 1), (3, 1), (3, 2), (4, 2)]
        );
      }
    };
  }

  #[test]
  fn complete() {
    let mut complete = false;
    {
      let s1 = Subject::default();
      let s2 = Subject::default();
      s1.clone()
        .zip(s2.clone())
        .on_complete(|| complete = true)
        .subscribe(|((), ())| {});

      s1.complete();
    }
    assert!(!complete);

    {
      let s1 = Subject::default();
      let s2 = Subject::default();
      s1.clone()
        .zip(s2.clone())
        .on_complete(|| complete = true)
        .subscribe(|((), ())| {});

      s1.complete();
      s2.complete();
    }
    assert!(complete);
  }
}
