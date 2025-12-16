//! Of operator implementation
//!
//! This module contains the Of operator, which is the simplest creation
//! operator. It emits a single value and completes.

use std::convert::Infallible;

use crate::{context::Context, observable::ObservableType, observer::Observer};

/// Of operator: Emits a single value and completes
///
/// This is the simplest creation operator that demonstrates:
/// - Direct subscription (no upstream source)
/// - Observer lifecycle (next → complete)
/// - Returning () from subscription (optimization for synchronous completion)
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::of(42);
/// let mut result = None;
/// observable.subscribe(|v| {
///   result = Some(v);
/// });
/// assert_eq!(result, Some(42));
/// ```
#[derive(Clone)]
pub struct Of<T>(pub T);

impl<T> ObservableType for Of<T> {
  type Item<'a>
    = T
  where
    T: 'a;
  type Err = Infallible;
}

impl<C, T> crate::observable::CoreObservable<C> for Of<T>
where
  C: Context,
  C::Inner: Observer<T, Infallible>,
  T: Clone,
{
  type Unsub = ();

  fn subscribe(self, context: C) -> Self::Unsub {
    // The verification file shows using () even though the observer expects
    // Infallible This suggests that the () value is never actually used
    let mut observer = context.into_inner();
    observer.next(self.0);
    observer.complete();
  }
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_of_emits_value() {
    use std::{cell::RefCell, rc::Rc};

    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::of(42).subscribe(move |v| {
      *result_clone.borrow_mut() = Some(v);
    });
    assert_eq!(*result.borrow(), Some(42));
  }

  #[rxrust_macro::test(local)]
  async fn test_of_with_different_types() {
    use std::{cell::RefCell, rc::Rc};

    let string_result = Rc::new(RefCell::new(None));
    let string_result_clone = string_result.clone();
    Local::of("hello".to_string()).subscribe(move |v| {
      *string_result_clone.borrow_mut() = Some(v);
    });
    assert_eq!(*string_result.borrow(), Some("hello".to_string()));

    let bool_result = Rc::new(RefCell::new(None));
    let bool_result_clone = bool_result.clone();
    Local::of(true).subscribe(move |v| {
      *bool_result_clone.borrow_mut() = Some(v);
    });
    assert_eq!(*bool_result.borrow(), Some(true));
  }
}
