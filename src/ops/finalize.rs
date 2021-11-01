use crate::prelude::*;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct FinalizeOp<S, F> {
  pub(crate) source: S,
  pub(crate) func: F,
}

impl<S, F> Observable for FinalizeOp<S, F>
where
  S: Observable,
  F: FnMut(),
{
  type Item = S::Item;
  type Err = S::Err;
}

impl<'a, S, F> LocalObservable<'a> for FinalizeOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut() + 'static,
{
  type Unsub = FinalizerSubscription<S::Unsub, Rc<RefCell<Option<F>>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    let func = Rc::new(RefCell::new(Some(self.func)));
    let subscription = self.source.actual_subscribe(FinalizerObserver {
      observer,
      func: func.clone(),
    });

    FinalizerSubscription { subscription, func }
  }
}

impl<S, F> SharedObservable for FinalizeOp<S, F>
where
  S: SharedObservable,
  F: FnMut() + Send + Sync + 'static,
  S::Unsub: Send + Sync,
{
  type Unsub = FinalizerSubscription<S::Unsub, Arc<Mutex<Option<F>>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let func = Arc::new(Mutex::new(Some(self.func)));
    let subscription = self.source.actual_subscribe(FinalizerObserver {
      observer,
      func: func.clone(),
    });
    FinalizerSubscription { subscription, func }
  }
}

struct FinalizerObserver<O, F> {
  observer: O,
  func: F,
}

pub struct FinalizerSubscription<U, F> {
  subscription: U,
  func: F,
}

impl<Target, U> SubscriptionLike
  for FinalizerSubscription<U, Arc<Mutex<Option<Target>>>>
where
  Target: FnMut(),
  U: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self.subscription.unsubscribe();
    if let Some(mut func) = (self.func.lock().unwrap()).take() {
      func()
    }
  }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<Target, U> SubscriptionLike
  for FinalizerSubscription<U, Rc<RefCell<Option<Target>>>>
where
  Target: FnMut(),
  U: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self.subscription.unsubscribe();
    if let Some(mut func) = (self.func.borrow_mut()).take() {
      func()
    }
  }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<Target, U> SubscriptionLike
  for FinalizerSubscription<U, Box<Option<Target>>>
where
  Target: FnMut(),
  U: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self.subscription.unsubscribe();
    if let Some(mut func) = (self.func).take() {
      func()
    }
  }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<Item, Err, O, Target> Observer
  for FinalizerObserver<O, Arc<Mutex<Option<Target>>>>
where
  O: Observer<Item = Item, Err = Err>,
  Target: FnMut(),
{
  type Item = Item;
  type Err = Err;
  #[inline]
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(&mut self, err: Err) {
    self.observer.error(err);
    if let Some(mut func) = (self.func.lock().unwrap()).take() {
      func()
    }
  }

  fn complete(&mut self) {
    self.observer.complete();
    if let Some(mut func) = (self.func.lock().unwrap()).take() {
      func()
    }
  }
}

impl<Item, Err, O, Target> Observer
  for FinalizerObserver<O, Rc<RefCell<Option<Target>>>>
where
  O: Observer<Item = Item, Err = Err>,
  Target: FnMut(),
{
  type Item = Item;
  type Err = Err;
  #[inline]
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(&mut self, err: Err) {
    self.observer.error(err);
    if let Some(mut func) = (self.func.borrow_mut()).take() {
      func()
    }
  }

  fn complete(&mut self) {
    self.observer.complete();
    if let Some(mut func) = (self.func.borrow_mut()).take() {
      func()
    }
  }
}

impl<Item, Err, O, Target> Observer
  for FinalizerObserver<O, Box<Option<Target>>>
where
  O: Observer<Item = Item, Err = Err>,
  Target: FnMut(),
{
  type Item = Item;
  type Err = Err;
  #[inline]
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(&mut self, err: Err) {
    self.observer.error(err);
    if let Some(mut func) = (self.func).take() {
      func()
    }
  }

  fn complete(&mut self) {
    self.observer.complete();
    if let Some(mut func) = (self.func).take() {
      func()
    }
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
    let mut s = LocalSubject::new();
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
    let mut s = LocalSubject::new();
    // When
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let mut subscription = s
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
    let mut s: LocalSubject<i32, &'static str> = LocalSubject::new();
    // When
    let finalized_clone = finalized.clone();
    let nexted_clone = nexted.clone();
    let errored_clone = errored.clone();
    s.clone()
      .finalize(move || finalized_clone.set(true))
      .subscribe_err(
        move |_| nexted_clone.set(true),
        move |_| errored_clone.set(true),
      );
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
    let mut s: LocalSubject<i32, &'static str> = LocalSubject::new();
    // When
    let finalized_clone = finalize_count.clone();
    let mut subscription = s
      .clone()
      .finalize(move || finalized_clone.set(finalized_clone.get() + 1))
      .subscribe_err(|_| (), |_| ());
    s.next(1);
    s.next(2);
    s.error("oops");
    s.complete();
    subscription.unsubscribe();
    // Then
    assert_eq!(finalize_count.get(), 1);
  }

  #[test]
  fn finalize_shared() {
    // Given
    let finalized = Arc::new(AtomicBool::new(false));
    let mut s = SharedSubject::new();
    // When
    let finalized_clone = finalized.clone();
    let mut subscription = s
      .clone()
      .into_shared()
      .finalize(move || finalized_clone.store(true, Ordering::Relaxed))
      .into_shared()
      .subscribe(|_| ());
    s.next(1);
    s.next(2);
    subscription.unsubscribe();
    // Then
    assert!(finalized.load(Ordering::Relaxed));
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_finalize);

  fn bench_finalize(b: &mut bencher::Bencher) {
    b.iter(finalize_on_complete_simple);
  }
}
