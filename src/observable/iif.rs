//! Iif: choose one of two observables at subscribe time.

use crate::{
  observable::{CoreObservable, ObservableType},
  subscription::EitherSubscription,
};

/// Subscribes `then_source` when `condition()` is true, else `else_source`.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::iif(|| 2 > 1, Local::of("yes"), Local::of("no")).subscribe(|v| result.push(v));
/// assert_eq!(result, vec!["yes"]);
/// ```
#[derive(Clone)]
pub struct Iif<F, A, B> {
  pub condition: F,
  pub then_source: A,
  pub else_source: B,
}

impl<F, A, B> ObservableType for Iif<F, A, B>
where
  A: ObservableType,
{
  type Item<'a>
    = A::Item<'a>
  where
    Self: 'a;
  type Err = A::Err;
}

impl<F, A, B, C> CoreObservable<C> for Iif<F, A, B>
where
  F: FnOnce() -> bool,
  A: CoreObservable<C>,
  B: CoreObservable<C>,
{
  type Unsub = EitherSubscription<A::Unsub, B::Unsub>;

  fn subscribe(self, context: C) -> Self::Unsub {
    if (self.condition)() {
      EitherSubscription::Left(self.then_source.subscribe(context))
    } else {
      EitherSubscription::Right(self.else_source.subscribe(context))
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  fn run(flag: bool) -> Vec<i32> {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    Local::iif(move || flag, Local::from_iter(vec![1, 2]), Local::from_iter(vec![9]))
      .subscribe(move |v| result_c.borrow_mut().push(v));
    result.borrow().clone()
  }

  #[rxrust_macro::test]
  fn test_iif_then() {
    assert_eq!(run(true), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_iif_else() {
    assert_eq!(run(false), vec![9]);
  }

  #[rxrust_macro::test]
  fn test_iif_evaluates_at_subscribe() {
    let calls = Rc::new(RefCell::new(0));
    let calls_c = calls.clone();
    let observable = Local::iif(
      move || {
        *calls_c.borrow_mut() += 1;
        true
      },
      Local::of(1),
      Local::of(2),
    );
    assert_eq!(*calls.borrow(), 0);
    observable.subscribe(|_| {});
    assert_eq!(*calls.borrow(), 1);
  }
}
