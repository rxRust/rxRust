//! Contains operator implementation
//!
//! This module contains the Contains operator, which emits a boolean indicating
//! whether a specific value is emitted by the source Observable.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, single_assignment::State},
};

/// Contains operator: Checks if the source observable emits a specific value
///
/// This operator compares each emitted item with a target value. If a match is
/// found, it emits `true` and completes immediately. If the source completes
/// without finding the value, it emits `false` and completes.
///
/// # Type Parameters
///
/// * `S` - The source observable type
/// * `Item` - The type of item being searched for (must implement PartialEq)
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]);
///
/// // Check for existing value
/// let mut found = false;
/// observable
///   .clone()
///   .contains(3)
///   .subscribe(|v| found = v);
/// assert!(found);
///
/// // Check for missing value
/// let mut found = true;
/// observable.contains(10).subscribe(|v| found = v);
/// assert!(!found);
/// ```
#[derive(Clone)]
pub struct Contains<S, Item> {
  pub source: S,
  pub target: Item,
}

impl<S, Item> ObservableType for Contains<S, Item>
where
  S: ObservableType,
{
  type Item<'a>
    = bool
  where
    Self: 'a;
  type Err = S::Err;
}

/// ContainsObserver wrapper for checking if an item exists
pub struct ContainsObserver<O, Item, U> {
  active: Option<(U, O)>,
  target: Item,
}

impl<O, U, Item, Err> Observer<Item, Err> for ContainsObserver<O, Item, U>
where
  U: Subscription,
  O: Observer<bool, Err>,
  Item: PartialEq,
{
  fn next(&mut self, v: Item) {
    if v == self.target
      && let Some((upstream, mut observer)) = self.active.take()
    {
      observer.next(true);
      observer.complete();
      upstream.unsubscribe();
    }
  }

  fn error(self, e: Err) {
    if let Some((_upstream, observer)) = self.active {
      observer.error(e);
    }
  }

  fn complete(self) {
    if let Some((_upstream, mut observer)) = self.active {
      observer.next(false);
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .active
      .as_ref()
      .is_none_or(|(_, observer)| observer.is_closed())
  }
}

type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;

impl<S, C, Item, U> CoreObservable<C> for Contains<S, Item>
where
  C: Context,
  U: Subscription,
  Item: PartialEq,
  S: CoreObservable<C::With<ContainsObserver<C::Inner, Item, ()>>, Unsub = U>
    + CoreObservable<C::With<ContainsObserver<C::Inner, Item, Handle<C, U>>>, Unsub = U>,
{
  type Unsub = Handle<C, U>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let (install_upstream, [upstream, subscription]) = Handle::<C, U>::channel();
    let wrapped = context.transform(|observer| ContainsObserver {
      active: Some((upstream, observer)),
      target: self.target,
    });
    install_upstream(self.source.subscribe(wrapped));
    subscription
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn matching_value_cancels_late_handle() {
    use crate::subscription::ClosureSubscription;
    let calls = Rc::new(std::cell::Cell::new(0));
    let c = calls.clone();
    Local::create(move |e| {
      e.next(1);
      ClosureSubscription(move || c.set(c.get() + 1))
    })
    .contains(1)
    .subscribe(|v| assert!(v));
    assert_eq!(calls.get(), 1);
  }
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_contains_found() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .contains(3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![true]);
  }

  #[rxrust_macro::test]
  fn test_contains_not_found() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .contains(10)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![false]);
  }

  #[rxrust_macro::test]
  fn test_contains_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([])
      .contains(1)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![false]);
  }

  #[rxrust_macro::test]
  fn test_contains_short_circuit() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let items_emitted = Rc::new(RefCell::new(0));
    let result_clone = result.clone();
    let items_emitted_clone = items_emitted.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .tap(move |_| *items_emitted_clone.borrow_mut() += 1)
      .contains(3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![true]);
    // Should process 1, 2, 3 and then stop
    assert_eq!(*items_emitted.borrow(), 3);
  }

  #[rxrust_macro::test]
  fn test_contains_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .map(|_| 0)
      .contains(5)
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<bool>::new());
    assert_eq!(*error.borrow(), "test error");
  }
}
