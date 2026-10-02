//! Timestamp operator implementation
//!
//! Attaches the wall-clock instant of emission to each item.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::Instant,
};

/// An item together with the instant it was emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamped<T> {
  /// The emitted value
  pub value: T,
  /// When the value passed through the `timestamp` operator
  pub timestamp: Instant,
}

/// Timestamp operator: Wraps each item in a [`Timestamped`]
///
/// Uses [`Instant::now`] from the scheduler module, so it works on wasm.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 2])
///   .timestamp()
///   .subscribe(|t| result.push(t.value));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[derive(Clone)]
pub struct Timestamp<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for Timestamp<S> {
  type Item<'a>
    = Timestamped<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that stamps each item
pub struct TimestampObserver<O> {
  observer: O,
}

impl<O, Item, Err> Observer<Item, Err> for TimestampObserver<O>
where
  O: Observer<Timestamped<Item>, Err>,
{
  fn next(&mut self, value: Item) {
    self
      .observer
      .next(Timestamped { value, timestamp: Instant::now() });
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> CoreObservable<C> for Timestamp<S>
where
  C: Context,
  S: CoreObservable<C::With<TimestampObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped = context.transform(|observer| TimestampObserver { observer });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_timestamp_preserves_values_and_orders_time() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let before = Instant::now();

    Local::from_iter([1, 2, 3])
      .timestamp()
      .subscribe(move |t| result_c.borrow_mut().push(t));

    let after = Instant::now();
    let stamped = result.borrow();
    assert_eq!(
      stamped
        .iter()
        .map(|t| t.value)
        .collect::<Vec<_>>(),
      vec![1, 2, 3]
    );
    for pair in stamped.windows(2) {
      assert!(pair[0].timestamp <= pair[1].timestamp);
    }
    assert!(stamped[0].timestamp >= before);
    assert!(stamped[2].timestamp <= after);
  }

  #[rxrust_macro::test]
  fn test_timestamp_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .timestamp()
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
