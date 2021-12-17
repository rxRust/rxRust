use crate::{impl_helper::*, impl_local_shared_both, prelude::*};

/// An Observable that combines from two other two Observables.
///
/// This struct is created by the with_latest_from method on
/// [Observable](Observable::with_latest_from). See its documentation for more.
#[derive(Clone)]
pub struct WithLatestFromOp<A, B> {
  pub(crate) a: A,
  pub(crate) b: B,
}

impl<A, B> Observable for WithLatestFromOp<A, B>
where
  A: Observable,
  B: Observable<Err = A::Err>,
{
  type Item = (A::Item, B::Item);
  type Err = A::Err;
}

impl_local_shared_both! {
  impl<A, B> WithLatestFromOp<A, B>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let sub = $ctx::RcMultiSubscription::default();
    let o_with_latest_from =
      WithLatestFromObserver::new($observer, sub.clone());
    let o_with_latest_from = $ctx::Rc::own(o_with_latest_from);
    sub.add(
      $self
        .a
        .actual_subscribe(AObserver(o_with_latest_from.clone(),
          TypeHint::new())),
    );

    sub.add($self.b.actual_subscribe(BObserver(o_with_latest_from,
      TypeHint::new())));
    sub
  }
  where
    A: @ctx::Observable,
    B: @ctx::Observable<Err = A::Err>,
    @ctx::shared_only(
      A::Item: Send + Sync + 'static,
      B::Item: Send + Sync + Clone + 'static,
    )
    @ctx::local_only(
      A::Item: 'o,
      B::Item: Clone + 'o,
    )
    A::Unsub: 'static @ctx::shared_only(+ Send + Sync),
    B::Unsub: 'static @ctx::shared_only(+ Send + Sync + Clone)
}

enum WithLatestFromItem<A, B> {
  ItemA(A),
  ItemB(B),
}

struct WithLatestFromObserver<O, U, A, B> {
  observer: O,
  subscription: U,
  _a: Option<A>,
  b: Option<B>,
  completed_one: bool,
}

impl<O, U, A, B> WithLatestFromObserver<O, U, A, B> {
  fn new(o: O, u: U) -> Self {
    WithLatestFromObserver {
      observer: o,
      subscription: u,
      _a: None,
      b: None,
      completed_one: false,
    }
  }
}

impl<O, U, A, B, Err> Observer for WithLatestFromObserver<O, U, A, B>
where
  O: Observer<Item = (A, B), Err = Err>,
  U: SubscriptionLike,
  B: Clone,
{
  type Item = WithLatestFromItem<A, B>;
  type Err = Err;
  fn next(&mut self, value: WithLatestFromItem<A, B>) {
    match value {
      WithLatestFromItem::ItemA(v) => {
        if self.b.is_some() {
          self.observer.next((v, self.b.clone().unwrap()));
        }
      }
      WithLatestFromItem::ItemB(v) => self.b = Some(v),
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
  O: Observer<Item = WithLatestFromItem<A, B>, Err = Err>,
{
  type Item = A;
  type Err = Err;
  fn next(&mut self, value: A) {
    self.0.next(WithLatestFromItem::ItemA(value));
  }

  fn error(&mut self, err: Self::Err) { self.0.error(err) }

  fn complete(&mut self) { self.0.complete() }
}

struct BObserver<O, A>(O, TypeHint<A>);

impl<O, A, B, Err> Observer for BObserver<O, A>
where
  O: Observer<Item = WithLatestFromItem<A, B>, Err = Err>,
{
  type Item = B;
  type Err = Err;
  fn next(&mut self, value: B) {
    self.0.next(WithLatestFromItem::ItemB(value));
  }

  fn error(&mut self, err: Self::Err) { self.0.error(err) }

  fn complete(&mut self) { self.0.complete() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[test]
  fn simple() {
    let mut ret = String::new();

    {
      let mut s1 = LocalSubject::new();
      let mut s2 = LocalSubject::new();

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
    let with_latest_from = observable::from_iter(0..10)
      .with_latest_from(observable::from_iter(0..10));
    let with_latest_from_count = Arc::new(AtomicUsize::new(0));
    let wlfcc = with_latest_from_count.clone();
    with_latest_from
      .clone()
      .count()
      .subscribe(|v| with_latest_from_count.store(v, Ordering::Relaxed));
    let mut with_latest_fromed_sum = 0;
    assert_eq!(wlfcc.load(Ordering::Relaxed), 0);
    with_latest_from
      .map(|(a, b)| a + b)
      .sum()
      .subscribe(|v| with_latest_fromed_sum = v);
    assert_eq!(with_latest_fromed_sum, 0);
  }

  #[test]
  fn complete() {
    let mut complete = false;
    {
      let mut s1 = LocalSubject::new();
      s1.clone()
        .with_latest_from(LocalSubject::new())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
    }
    assert!(!complete);

    {
      let mut s1 = LocalSubject::new();
      let mut s2 = LocalSubject::new();
      s1.clone()
        .with_latest_from(s2.clone())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
      s2.complete();
    }
    assert!(complete);
  }

  #[test]
  fn circular() {
    let mut subject_a = LocalSubject::new();
    let mut subject_b = LocalSubject::new();
    let mut cloned_subject_b = subject_b.clone();

    subject_a.clone().with_latest_from(subject_b.clone()).subscribe(move |_| {
      cloned_subject_b.next(());
    });
    subject_b.next(());
    subject_a.next(());
    subject_a.next(());
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_zip);

  fn bench_zip(b: &mut bencher::Bencher) { b.iter(smoke); }
}
