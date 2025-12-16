use super::Subscription;

/// TupleSubscription that combines two subscriptions
///
/// This is a simple tuple-based subscription designed specifically for merge
/// operators and other multi-source scenarios in the v1 architecture. It
/// provides a straightforward way to manage two concurrent subscriptions
/// without the complexity of reference counting.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// // Merge operator usage
/// let obs1 = Local::from_iter([1, 3, 5]);
/// let obs2 = Local::from_iter([2, 4, 6]);
/// let merged = obs1.merge(obs2);
/// let subscription = merged.subscribe(|_| {});
///
/// // The TupleSubscription handles cleanup automatically
/// drop(subscription);
/// ```
pub struct TupleSubscription<U1, U2> {
  unsub1: U1,
  unsub2: U2,
}

impl<U1, U2> TupleSubscription<U1, U2> {
  /// Create a new tuple subscription for multi-source operators
  pub fn new(unsub1: U1, unsub2: U2) -> Self { TupleSubscription { unsub1, unsub2 } }
}

impl<U1, U2> Subscription for TupleSubscription<U1, U2>
where
  U1: Subscription,
  U2: Subscription,
{
  fn unsubscribe(self) {
    // Unsubscribe from both source subscriptions
    self.unsub1.unsubscribe();
    self.unsub2.unsubscribe();
  }

  fn is_closed(&self) -> bool {
    // Return true only when both subscriptions are closed
    self.unsub1.is_closed() && self.unsub2.is_closed()
  }
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
  fn test_tuple_subscription() {
    let (mock1, closed1) = MockSubscription::new();
    let (mock2, closed2) = MockSubscription::new();

    let tuple_sub = TupleSubscription::new(mock1, mock2);

    assert!(!tuple_sub.is_closed());
    tuple_sub.unsubscribe();

    assert!(*closed1.borrow());
    assert!(*closed2.borrow());
  }
}
