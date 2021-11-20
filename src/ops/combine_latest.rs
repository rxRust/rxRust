use crate::{impl_helper::*, impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct CombineLatestOp<A, B, BinaryOp> {
  pub(crate) a: A,
  pub(crate) b: B,
  pub(crate) binary_op: BinaryOp,
}

impl<A, B, BinaryOp, OutputItem> Observable for CombineLatestOp<A, B, BinaryOp>
where
  A: Observable,
  B: Observable<Err = A::Err>,
  BinaryOp: FnMut(A::Item, B::Item) -> OutputItem,
{
  type Item = OutputItem;
  type Err = A::Err;
}

impl_local_shared_both! {
  impl<A, B, BinaryOp, OutputItem>  CombineLatestOp<A, B, BinaryOp>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let sub = $ctx::RcMultiSubscription::default();
    let o_combine = CombineLatestObserver::new(
      $observer,
      sub.clone(),
      $self.binary_op,
    );
    let o_combine = $ctx::Rc::own(o_combine);
    sub.add(
      $self.a.actual_subscribe(AObserver(o_combine.clone(), TypeHint::new()))
    );
    sub.add($self.b.actual_subscribe(BObserver(o_combine, TypeHint::new())));
    sub
  }
  where
    A: @ctx::Observable,
    B: @ctx::Observable<Err = A::Err>,
    BinaryOp: FnMut(A::Item, B::Item) -> OutputItem +
      @ctx::local_only('o)
      @ctx::shared_only(Send + Sync + 'static),
    A::Item: Clone
      + @ctx::local_only('o) @ctx::shared_only(Send + Sync + 'static),
    B::Item: Clone
      + @ctx::local_only('o) @ctx::shared_only(Send + Sync + 'static),
    A::Unsub: 'static,
    B::Unsub: 'static
}

enum CombineItem<A, B> {
  ItemA(A),
  ItemB(B),
}

struct CombineLatestObserver<O, U, A, B, BinaryOp> {
  observer: O,
  subscription: U,
  a: Option<A>,
  b: Option<B>,
  binary_op: BinaryOp,
  completed_one: bool,
}

impl<O, U, A, B, BinaryOp> CombineLatestObserver<O, U, A, B, BinaryOp> {
  fn new(o: O, u: U, binary_op: BinaryOp) -> Self {
    CombineLatestObserver {
      observer: o,
      subscription: u,
      a: None,
      b: None,
      binary_op,
      completed_one: false,
    }
  }
}

impl<O, U, A, B, OutputItem, BinaryOp, Err> Observer
  for CombineLatestObserver<O, U, A, B, BinaryOp>
where
  O: Observer<Item = OutputItem, Err = Err>,
  U: SubscriptionLike,
  BinaryOp: FnMut(A, B) -> OutputItem,
  A: Clone,
  B: Clone,
{
  type Item = CombineItem<A, B>;
  type Err = Err;
  fn next(&mut self, value: CombineItem<A, B>) {
    match value {
      CombineItem::ItemA(v) => {
        self.a = Some(v);
      }
      CombineItem::ItemB(v) => {
        self.b = Some(v);
      }
    }
    if let (Some(a), Some(b)) = (self.a.clone(), self.b.clone()) {
      self.observer.next((self.binary_op)(a, b));
    }
  }

  fn error(&mut self, err: Err) {
    self.observer.error(err);
    self.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    if self.completed_one {
      self.subscription.unsubscribe();
      self.observer.complete();
    } else {
      self.completed_one = true;
    }
  }
}

struct AObserver<O, B>(O, TypeHint<B>);

impl<O, A, B, Err> Observer for AObserver<O, B>
where
  O: Observer<Item = CombineItem<A, B>, Err = Err>,
{
  type Item = A;
  type Err = Err;
  #[inline]
  fn next(&mut self, value: A) { self.0.next(CombineItem::ItemA(value)); }

  #[inline]
  fn error(&mut self, err: Self::Err) { self.0.error(err) }

  #[inline]
  fn complete(&mut self) { self.0.complete() }
}

struct BObserver<O, A>(O, TypeHint<A>);

impl<O, A, B, Err> Observer for BObserver<O, A>
where
  O: Observer<Item = CombineItem<A, B>, Err = Err>,
{
  type Item = B;
  type Err = Err;
  fn next(&mut self, value: B) { self.0.next(CombineItem::ItemB(value)); }

  #[inline]
  fn error(&mut self, err: Self::Err) { self.0.error(err) }

  #[inline]
  fn complete(&mut self) { self.0.complete() }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::time::Duration;

  use crate::test_scheduler::ManualScheduler;

  use super::*;

  #[test]
  fn combine_latest_base() {
    let scheduler = ManualScheduler::now();
    let x = Rc::new(RefCell::new(vec![]));

    let interval =
      observable::interval(Duration::from_millis(2), scheduler.clone());
    {
      let x_c = x.clone();
      interval
        .combine_latest(
          observable::interval(Duration::from_millis(3), scheduler.clone()),
          |a, b| (a, b),
        )
        .take(7)
        .subscribe(move |v| {
          x_c.borrow_mut().push(v);
        });

      scheduler.advance_and_run(Duration::from_millis(1), 10);
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
      let mut s1 = LocalSubject::new();
      let s2 = LocalSubject::new();
      s1.clone()
        .zip(s2.clone())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
    }
    assert!(!complete);

    {
      let mut s1 = LocalSubject::new();
      let mut s2 = LocalSubject::new();
      s1.clone()
        .zip(s2.clone())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
      s2.complete();
    }
    assert!(complete);
  }
}
