//! SkipUntil operator implementation
//!
//! This module contains the SkipUntil operator, which skips values from the
//! source Observable until a second Observable (the notifier) emits a value.

use crate::{
  context::{Context, SharedCell},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, TupleSubscription, single_assignment::State},
};

/// SkipUntil operator
///
/// Skips the values emitted by the source Observable until a `notifier`
/// Observable emits a value. Once the notifier emits, all subsequent source
/// values are passed through.
#[derive(Clone)]
pub struct SkipUntil<S, N> {
  pub source: S,
  pub notifier: N,
}

/// Observer for the source observable
pub struct SkipUntilObserver<O, SkipState, NProxy> {
  observer: O,
  skip_state: SkipState,
  notifier_proxy: NProxy,
  notifier_done: SkipState,
}

/// Observer for the notifier observable
///
/// When the notifier emits, it sets the skip state to false, allowing
/// values from the source to pass through.
pub struct SkipUntilNotifierObserver<SkipState, H> {
  upstream: H,
  done: SkipState,
  skip_state: SkipState,
}

impl<S, N> ObservableType for SkipUntil<S, N>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
impl<S, N, C, V> CoreObservable<C> for SkipUntil<S, N>
where
  C: Context,
  V: Subscription,
  N: CoreObservable<C::With<SkipUntilNotifierObserver<C::RcCell<bool>, ()>>, Unsub = V>
    + CoreObservable<C::With<SkipUntilNotifierObserver<C::RcCell<bool>, Handle<C, V>>>, Unsub = V>,
  S: CoreObservable<C::With<SkipUntilObserver<C::Inner, C::RcCell<bool>, Handle<C, V>>>>,
{
  type Unsub = TupleSubscription<S::Unsub, Handle<C, V>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let skip_state = C::RcCell::from(true);
    let notifier_done = C::RcCell::from(false);
    let notifier = Handle::<C, V>::new();
    notifier.set(
      self
        .notifier
        .subscribe(context.wrap(SkipUntilNotifierObserver {
          skip_state: skip_state.clone(),
          done: notifier_done.clone(),
          upstream: notifier.clone(),
        })),
    );
    let source = self
      .source
      .subscribe(context.transform(|observer| SkipUntilObserver {
        observer,
        skip_state,
        notifier_done,
        notifier_proxy: notifier.clone(),
      }));
    TupleSubscription::new(source, notifier)
  }
}

// Implement Observer for Source
impl<Item, Err, O, SkipState, NProxy> Observer<Item, Err>
  for SkipUntilObserver<O, SkipState, NProxy>
where
  O: Observer<Item, Err>,
  SkipState: SharedCell<bool>,
  NProxy: Subscription,
{
  fn next(&mut self, value: Item) {
    // Only emit if we're no longer skipping
    if !self.skip_state.get() {
      self.observer.next(value);
    }
  }

  fn error(self, err: Err) {
    self.observer.error(err);
    if !self.notifier_done.get() {
      self.notifier_proxy.unsubscribe();
    }
  }

  fn complete(self) {
    self.observer.complete();
    if !self.notifier_done.get() {
      self.notifier_proxy.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

// Implement Observer for Notifier
impl<NotifyItem, NotifyErr, SkipState, H> Observer<NotifyItem, NotifyErr>
  for SkipUntilNotifierObserver<SkipState, H>
where
  SkipState: SharedCell<bool>,
  H: Subscription + Clone,
{
  fn next(&mut self, _value: NotifyItem) {
    // Stop skipping when notifier emits
    self.skip_state.set(false);
    self.done.set(true);
    self.upstream.clone().unsubscribe();
  }

  fn error(self, _err: NotifyErr) {
    self.done.set(true);
    // Ignore errors from notifier as per legacy behavior
  }

  fn complete(self) {
    self.done.set(true);
    // When notifier completes without emitting, also stop skipping
    // (as per legacy behavior where complete triggers stop_skipping)
    self.skip_state.set(false);
  }

  fn is_closed(&self) -> bool {
    // Closed when no longer skipping
    !self.skip_state.get()
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn synchronous_notifier_cancels_late_handle() {
    use crate::subscription::ClosureSubscription;
    let calls = Rc::new(std::cell::Cell::new(0));
    let c = calls.clone();
    let notifier = Local::create::<(), Infallible, _, _>(move |e| {
      e.next(());
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    let mut values = vec![];
    Local::of(1)
      .skip_until(notifier)
      .subscribe(|v| values.push(v));
    assert_eq!(values, vec![1]);
    assert_eq!(calls.get(), 1);
  }
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_skip_until_skips_before_notifier_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .skip_until(notifier.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    notifier.next(()); // Now start emitting
    source.next(3);
    source.next(4);

    assert_eq!(*result.borrow(), vec![3, 4]);
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_until_with_from_iter_and_tap() {
    let completed = Rc::new(RefCell::new(false));
    let items = Rc::new(RefCell::new(Vec::new()));
    let completed_clone = completed.clone();
    let items_clone = items.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let notifier_clone = notifier.clone();

    Local::from_iter(0..10)
      .map(move |v| {
        if v == 5 {
          notifier.next(());
        }
        v
      })
      .skip_until(notifier_clone)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        items_clone.borrow_mut().push(v);
      });

    assert_eq!(&*items.borrow(), &[5, 6, 7, 8, 9]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_until_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .skip_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    source.complete();

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_until_notifier_complete_opens_gate() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .skip_until(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.next(1);
    notifier.complete(); // Complete should also open the gate
    source.next(2);

    assert_eq!(*result.borrow(), vec![2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_skip_until_support_fork() {
    let items1 = Rc::new(RefCell::new(Vec::new()));
    let items2 = Rc::new(RefCell::new(Vec::new()));
    let items1_clone = items1.clone();
    let items2_clone = items2.clone();

    // Use a subject-based notifier with simple from_iter source
    let notifier = Local::subject::<(), Infallible>();
    let skip_until = Local::from_iter(0..10).skip_until(notifier);

    // Clone the skip_until before subscribe
    let fork1 = skip_until.clone();
    let fork2 = skip_until;

    fork1.subscribe(move |v| items1_clone.borrow_mut().push(v));
    fork2.subscribe(move |v| items2_clone.borrow_mut().push(v));

    // Both should have empty results since notifier never emitted
    assert_eq!(*items1.borrow(), Vec::<i32>::new());
    assert_eq!(*items2.borrow(), Vec::<i32>::new());
  }
}
