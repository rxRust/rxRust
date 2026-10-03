//! Emits source values until the notifier emits. Notifier errors and completion
//! are ignored; source termination releases an active notifier.
use crate::{
  context::{Context, RcDeref, SharedCell},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, TupleSubscription, single_assignment::State},
};
#[derive(Clone)]
pub struct TakeUntil<S, N> {
  pub source: S,
  pub notifier: N,
}
impl<S: ObservableType, N> ObservableType for TakeUntil<S, N> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}
pub struct TakeUntilObserver<O, H, F> {
  observer: O,
  notifier: H,
  notifier_done: F,
}
pub struct TakeUntilNotifierObserver<O, H, N, F> {
  observer: O,
  source: H,
  notifier: N,
  done: F,
  complete: fn(O),
}
type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type Down<C> = <C as Context>::RcMut<Option<<C as Context>::Inner>>;
impl<S, N, C, U, V> CoreObservable<C> for TakeUntil<S, N>
where
  C: Context,
  U: Subscription,
  V: Subscription,
  S: CoreObservable<C::With<TakeUntilObserver<Down<C>, (), C::RcCell<bool>>>, Unsub = U>
    + CoreObservable<C::With<TakeUntilObserver<Down<C>, Handle<C, V>, C::RcCell<bool>>>, Unsub = U>,
  N: CoreObservable<C::With<TakeUntilNotifierObserver<Down<C>, (), (), C::RcCell<bool>>>, Unsub = V>
    + CoreObservable<
      C::With<TakeUntilNotifierObserver<Down<C>, Handle<C, U>, Handle<C, V>, C::RcCell<bool>>>,
      Unsub = V,
    >,
  for<'a> Down<C>: Observer<S::Item<'a>, S::Err>,
{
  type Unsub = TupleSubscription<Handle<C, U>, Handle<C, V>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let (downstream, scheduler) = context.into_parts();
    let observer = C::RcMut::from(Some(downstream));
    let source = Handle::<C, U>::new();
    let notifier = Handle::<C, V>::new();
    let done = C::RcCell::from(false);
    let n = TakeUntilNotifierObserver {
      observer: observer.clone(),
      source: source.clone(),
      notifier: notifier.clone(),
      done: done.clone(),
      complete: |o| o.complete(),
    };
    notifier.set(
      self
        .notifier
        .subscribe(C::With::from_parts(n, scheduler.clone())),
    );
    if observer.rc_deref().is_some() {
      source.set(self.source.subscribe(C::With::from_parts(
        TakeUntilObserver { observer, notifier: notifier.clone(), notifier_done: done },
        scheduler,
      )));
    }
    TupleSubscription::new(source, notifier)
  }
}
impl<I, E, O, H, F> Observer<I, E> for TakeUntilObserver<O, H, F>
where
  O: Observer<I, E> + Clone,
  H: Subscription,
  F: SharedCell<bool>,
{
  fn next(&mut self, v: I) { self.observer.clone().next(v); }
  fn error(self, e: E) {
    self.observer.error(e);
    if !self.notifier_done.get() {
      self.notifier.unsubscribe();
    }
  }
  fn complete(self) {
    self.observer.complete();
    if !self.notifier_done.get() {
      self.notifier.unsubscribe();
    }
  }
  fn is_closed(&self) -> bool { self.observer.is_closed() }
}
impl<I, E, O, H, N, F, T> Observer<I, E> for TakeUntilNotifierObserver<O, H, N, F>
where
  O: RcDeref<Target = Option<T>>,
  H: Subscription + Clone,
  N: Subscription + Clone,
  F: SharedCell<bool>,
{
  fn next(&mut self, _: I) {
    if !self.done.get() {
      self.done.set(true);
      (self.complete)(self.observer.clone());
      self.source.clone().unsubscribe();
      self.notifier.clone().unsubscribe();
    }
  }
  fn error(self, _: E) { self.done.set(true); }
  fn complete(self) { self.done.set(true); }
  fn is_closed(&self) -> bool { self.done.get() || self.observer.rc_deref().is_none() }
}
#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn synchronous_notifier_cancels_itself_without_starting_source() {
    use crate::subscription::ClosureSubscription;
    let calls = Rc::new(std::cell::Cell::new(0));
    let c = calls.clone();
    let notifier = Local::create::<(), Infallible, _, _>(move |e| {
      e.next(());
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    Local::create::<i32, Infallible, _, ()>(|_| panic!("source started"))
      .take_until(notifier)
      .subscribe(|_| {});
    assert_eq!(calls.get(), 1);
  }
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_take_until_emits_until_notifier_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    notifier.next(());
    source.next(3);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    notifier.next(());

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_source_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    source.complete();

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_notifier_complete_does_nothing() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.next(1);
    notifier.complete(); // Should be ignored
    source.next(2);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
