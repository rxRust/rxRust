use crate::impl_helper::*;
use crate::impl_local_shared_both;
use crate::prelude::*;

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
    let subscription =  $ctx::RcMultiSubscription::default();

    let item = $ctx::Rc::own(None);
    let source_observer = $ctx::Rc::own($observer);
    subscription.add($self.b.actual_subscribe(BObserver {
      observer: source_observer.clone(),
      value: item.clone(),
      subscription: subscription.clone(),
      done: false,
    }));
    subscription.add($self.a.actual_subscribe(AObserver {
      observer: source_observer,
      value: item,
    }));
    subscription
  }
  where
    A: @ctx::Observable,
    B: @ctx::Observable<Err=A::Err>,
    A::Unsub: 'static,
    B::Unsub: 'static,
    A::Item: @ctx::local_only('o) @ctx::shared_only('static),
    B::Item: Clone
      @ctx::local_only(+ 'o)
      @ctx::shared_only(+ Send + Sync + 'static)
}

#[derive(Clone)]
struct BObserver<O, V, Unsub> {
  observer: O,
  value: V,
  subscription: Unsub,
  done: bool,
}

macro_rules! impl_b_observer {
  ($rc: ident) => {
    impl<O, AItem, BItem, Unsub> Observer
      for BObserver<O, $rc<Option<BItem>>, Unsub>
    where
      O: Observer<Item = (AItem, BItem)>,
      Unsub: SubscriptionLike,
    {
      type Item = BItem;
      type Err = O::Err;
      fn next(&mut self, value: Self::Item) {
        *self.value.rc_deref_mut() = Some(value);
      }

      fn error(&mut self, err: Self::Err) { self.observer.error(err) }

      fn complete(&mut self) {
        if !self.done {
          self.subscription.unsubscribe();
          self.done = true;
        }
      }
    }
  };
}

impl_b_observer!(MutRc);
impl_b_observer!(MutArc);

#[derive(Clone)]
struct AObserver<O: Observer, V> {
  observer: O,
  value: V,
}

macro_rules! impl_a_observer {
  ($rc: ident) => {
    impl<AItem, BItem, Err, O> Observer for AObserver<O, $rc<Option<BItem>>>
    where
      O: Observer<Item = (AItem, BItem), Err = Err>,
      BItem: Clone,
    {
      type Item = AItem;
      type Err = Err;

      fn next(&mut self, item: Self::Item) {
        let value = (*self.value.rc_deref()).clone();
        if value.is_none() {
          return;
        }
        let item2 = value.unwrap();
        self.observer.next((item, item2));
      }

      fn complete(&mut self) { self.observer.complete(); }

      fn error(&mut self, err: Self::Err) { self.observer.error(err) }
    }
  };
}

impl_a_observer!(MutRc);
impl_a_observer!(MutArc);

#[cfg(test)]
mod test {
  use crate::prelude::*;

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
    let mut a_store = vec![];
    let mut b_store = vec![];
    let mut numbers_store = vec![];

    {
      let mut numbers = LocalSubject::new();
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
      let mut s1 = LocalSubject::new();
      s1.clone()
        .with_latest_from(LocalSubject::new())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s1.complete();
    }
    assert!(complete);

    complete = false;
    {
      let s1 = LocalSubject::new();
      let mut s2 = LocalSubject::new();
      s1.clone()
        .with_latest_from(s2.clone())
        .subscribe_complete(|((), ())| {}, || complete = true);

      s2.complete();
    }
    assert!(!complete);
  }

  #[test]
  fn circular() {
    let mut subject_a = LocalSubject::new();
    let mut subject_b = LocalSubject::new();
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
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_zip);

  fn bench_zip(b: &mut bencher::Bencher) { b.iter(smoke); }
}
