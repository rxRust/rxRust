use super::Subscription;
use crate::context::{MutArc, MutRc, RcDeref, RcDerefMut};

/// EitherSubscription that holds one of two subscriptions
///
/// This enum is useful for operators that switch between two different
/// subscription states, such as `DelaySubscription` (TaskHandle ->
/// SourceSubscription).
#[derive(Default)]
pub enum EitherSubscription<A, B> {
  Left(A),
  Right(B),
  #[default]
  Idle,
}

impl<A, B> Subscription for EitherSubscription<A, B>
where
  A: Subscription,
  B: Subscription,
{
  fn unsubscribe(self) {
    match self {
      Self::Left(s) => s.unsubscribe(),
      Self::Right(s) => s.unsubscribe(),
      Self::Idle => {}
    }
  }

  fn is_closed(&self) -> bool {
    match self {
      Self::Left(s) => s.is_closed(),
      Self::Right(s) => s.is_closed(),
      Self::Idle => false,
    }
  }
}

// Special implementations for EitherSubscription wrapped in Rc/Arc
// Uses Idle variant as the "taken" state instead of Option wrapper
impl<A, B> Subscription for MutRc<EitherSubscription<A, B>>
where
  A: Subscription,
  B: Subscription,
{
  fn unsubscribe(self) {
    use std::mem::take;
    take(&mut *self.rc_deref_mut()).unsubscribe();
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_closed() }
}

impl<A, B> Subscription for MutArc<EitherSubscription<A, B>>
where
  A: Subscription,
  B: Subscription,
{
  fn unsubscribe(self) {
    use std::mem::take;
    take(&mut *self.rc_deref_mut()).unsubscribe();
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;

  /// A mock subscription for testing
  struct MockSubscription {
    closed: Rc<RefCell<bool>>,
  }

  impl MockSubscription {
    fn new() -> (Self, Rc<RefCell<bool>>) {
      let closed = Rc::new(RefCell::new(false));
      (Self { closed: closed.clone() }, closed)
    }
  }

  impl Subscription for MockSubscription {
    fn unsubscribe(self) { *self.closed.borrow_mut() = true; }

    fn is_closed(&self) -> bool { *self.closed.borrow() }
  }

  #[rxrust_macro::test]
  fn test_either_subscription() {
    let (mock, closed) = MockSubscription::new();
    let either: EitherSubscription<MockSubscription, ()> = EitherSubscription::Left(mock);

    assert!(!either.is_closed());
    either.unsubscribe();

    assert!(*closed.borrow());
  }
}
