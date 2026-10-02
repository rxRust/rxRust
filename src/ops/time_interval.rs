//! TimeInterval operator implementation
//!
//! Attaches the time elapsed since the previous emission to each item.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Instant},
};

/// An item together with the time elapsed since the previous item, or since
/// subscription for the first item.
#[doc(alias = "TimeInterval")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed<T> {
  /// The emitted value
  pub value: T,
  /// Time since the previous emission (or subscription)
  pub interval: Duration,
}

/// TimeInterval operator: Wraps each item in an [`Elapsed`]
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([1, 2])
///   .time_interval()
///   .subscribe(|e| result.push(e.value));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "timeInterval")]
#[derive(Clone)]
pub struct TimeInterval<S> {
  pub source: S,
}

impl<S: ObservableType> ObservableType for TimeInterval<S> {
  type Item<'a>
    = Elapsed<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Observer that measures the gap between emissions
pub struct TimeIntervalObserver<O> {
  observer: O,
  last: Instant,
}

impl<O, Item, Err> Observer<Item, Err> for TimeIntervalObserver<O>
where
  O: Observer<Elapsed<Item>, Err>,
{
  fn next(&mut self, value: Item) {
    let now = Instant::now();
    let interval = now.duration_since(self.last);
    self.last = now;
    self.observer.next(Elapsed { value, interval });
  }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, C> CoreObservable<C> for TimeInterval<S>
where
  C: Context,
  S: CoreObservable<C::With<TimeIntervalObserver<C::Inner>>>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let wrapped =
      context.transform(|observer| TimeIntervalObserver { observer, last: Instant::now() });
    self.source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_time_interval_preserves_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([1, 2, 3])
      .time_interval()
      .subscribe(move |e| result_c.borrow_mut().push(e.value));

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test(local)]
  async fn test_time_interval_measures_delay() {
    let result = Local::timer(Duration::from_millis(20))
      .time_interval()
      .into_future()
      .await;

    let elapsed = result.unwrap().unwrap();
    // A timer never fires early, so this lower bound is deterministic
    assert!(elapsed.interval >= Duration::from_millis(20));
  }

  #[rxrust_macro::test]
  fn test_time_interval_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_c = error.clone();

    Local::throw_err("boom".to_string())
      .map(|_| 0)
      .time_interval()
      .on_error(move |e| *error_c.borrow_mut() = e)
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "boom");
  }
}
