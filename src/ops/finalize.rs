use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDerefMut},
};

#[derive(Clone)]
pub struct FinalizeOp<S, F> {
  source: S,
  func: F,
}

#[derive(Clone)]
pub struct FinalizeOpThreads<S, F> {
  source: S,
  func: F,
}

macro_rules! impl_finalize_op {
  ($name: ident, $rc:ident) => {
    impl<S, F> $name<S, F> {
      #[inline]
      pub fn new(source: S, func: F) -> Self {
        Self { source, func }
      }
    }

    impl<Item, Err, O, S, F> Observable<Item, Err, O> for $name<S, F>
    where
      O: Observer<Item, Err>,
      S: Observable<Item, Err, FinalizerObserver<O, $rc<Option<F>>>>,
      F: FnOnce(),
    {
      type Unsub = FinalizerSubscription<S::Unsub, $rc<Option<F>>>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let func = $rc::own(Some(self.func));
        let subscription = self
          .source
          .actual_subscribe(FinalizerObserver { observer, func: func.clone() });
        FinalizerSubscription { subscription, func }
      }
    }

    impl<Item, Err, S, F> ObservableExt<Item, Err> for $name<S, F> where
      S: ObservableExt<Item, Err>
    {
    }
  };
}

impl_finalize_op!(FinalizeOp, MutRc);
impl_finalize_op!(FinalizeOpThreads, MutArc);
pub struct FinalizerObserver<O, F> {
  observer: O,
  func: F,
}

impl<Item, Err, O, F, C> Observer<Item, Err> for FinalizerObserver<O, C>
where
  C: RcDerefMut<Target = Option<F>>,
  O: Observer<Item, Err>,
  F: FnOnce(),
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next(value);
  }

  fn error(self, err: Err) {
    self.observer.error(err);
    if let Some(func) = self.func.rc_deref_mut().take() {
      func()
    }
  }

  fn complete(self) {
    self.observer.complete();
    if let Some(func) = self.func.rc_deref_mut().take() {
      func()
    }
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

pub struct FinalizerSubscription<U, F> {
  subscription: U,
  func: F,
}

impl<C, F, U> Subscription for FinalizerSubscription<U, C>
where
  U: Subscription,
  C: RcDerefMut<Target = Option<F>>,
  F: FnOnce(),
{
  fn unsubscribe(self) {
    self.subscription.unsubscribe();
    if let Some(func) = self.func.rc_deref_mut().take() {
      func()
    }
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.subscription.is_closed()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;
  use std::rc::Rc;
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  };

  #[test]
  fn finalize_on_complete_simple() {
    // Given
    let finalized = Rc::new(Cell::new(false));
    let mut nexted = false;
    let o = observable::of(1);
    // When
    let finalized_clone = finalized.clone();
    o.finalize(move || finalized_clone.set(true))
      .subscribe(|_| nexted = true);
    // Then
    assert!(finalized.get());
    assert!(nexted);
  }

  #[test]
  fn finalize_on_complete_subject() {
    // Given
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let mut s = Subject::default();
    // When
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    s.clone()
      .finalize(move || finalized_clone.set(true))
      .subscribe(move |_| nexted_clone.set(true));
    s.next(1);
    s.next(2);
    s.complete();
    // Then
    assert!(finalized.get());
    assert!(nexted.get());
  }

  #[test]
  fn finalize_on_unsubscribe() {
    // Given
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let mut s = Subject::default();
    // When
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let subscription = s
      .clone()
      .finalize(move || finalized_clone.set(true))
      .subscribe(move |_| nexted_clone.set(true));
    s.next(1);
    s.next(2);
    subscription.unsubscribe();
    // Then
    assert!(finalized.get());
    assert!(nexted.get());
  }

  #[test]
  fn finalize_on_error() {
    // Given
    let finalized = Rc::new(Cell::new(false));
    let nexted = Rc::new(Cell::new(false));
    let errored = Rc::new(Cell::new(false));
    let mut s: Subject<i32, &'static str> = Subject::default();
    // When
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let errored_clone = errored.clone();
    s.clone()
      .finalize(move || finalized_clone.set(true))
      .on_error(move |_| errored_clone.set(true))
      .subscribe(move |_| nexted_clone.set(true));
    s.next(1);
    s.next(2);
    s.error("oops");
    // Then
    assert!(finalized.get());
    assert!(errored.get());
    assert!(nexted.get());
  }

  #[test]
  fn finalize_only_once() {
    // Given
    let finalize_count = Rc::new(Cell::new(0));
    let mut s: Subject<i32, &'static str> = Subject::default();
    // When
    let finalized_clone = finalize_count.clone();
    let subscription = s
      .clone()
      .finalize(move || finalized_clone.set(finalized_clone.get() + 1))
      .on_error(|_| {})
      .subscribe(|_| {});
    s.next(1);
    s.next(2);
    s.error("oops");

    subscription.unsubscribe();
    // Then
    assert_eq!(finalize_count.get(), 1);
  }

  #[test]
  fn finalize_shared() {
    // Given
    let finalized = Arc::new(AtomicBool::new(false));
    let mut s = SubjectThreads::default();
    // When
    let finalized_clone = finalized.clone();
    let subscription = s
      .clone()
      .finalize_threads(move || finalized_clone.store(true, Ordering::Relaxed))
      .subscribe(|_| ());
    s.next(1);
    s.next(2);
    subscription.unsubscribe();
    // Then
    assert!(finalized.load(Ordering::Relaxed));
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_finalize);

  fn bench_finalize(b: &mut bencher::Bencher) {
    b.iter(finalize_on_complete_simple);
  }
}
