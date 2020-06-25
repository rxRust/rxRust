use crate::prelude::*;

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

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
self,
subscriber: Subscriber<O, $subscription>,
) -> Self::Unsub {
let mut subscription = subscriber.subscription.clone();
subscription.add(self.source.actual_subscribe(subscriber));
subscription.add(Finalizer {
s: <$subscription>::default(),
f: self.func,
});
subscription
}
}

impl<'a, S, F> LocalObservable<'a> for FinalizeOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut() + 'static,
{
  type Unsub = LocalSubscription;

  observable_impl!(LocalSubscription, 'a);
}

impl<S, F> SharedObservable for FinalizeOp<S, F>
where
  S: SharedObservable,
  F: FnMut() + Send + Sync + 'static,
  S::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;

  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

struct Finalizer<Sub, F> {
  s: Sub,
  f: F,
}

impl<Sub, F> SubscriptionLike for Finalizer<Sub, F>
where
  Sub: SubscriptionLike,
  F: FnMut(),
{
  fn unsubscribe(&mut self) {
    self.s.unsubscribe();
    (self.f)()
  }

  fn is_closed(&self) -> bool { self.s.is_closed() }

  fn inner_addr(&self) -> *const () { self.s.inner_addr() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;
  use std::rc::Rc;
  use std::sync::{Arc, Mutex};

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
    let finalized = Arc::new(Mutex::new(false));
    let mut s = SharedSubject::new();
    // When
    let finalized_clone = finalized.clone();
    let mut subscription = s
      .clone()
      .to_shared()
      .finalize(move || *finalized_clone.lock().unwrap() = true)
      .to_shared()
      .subscribe(|_| ());
    s.next(1);
    s.next(2);
    subscription.unsubscribe();
    // Then
    assert!(*finalized.lock().unwrap());
  }
}
