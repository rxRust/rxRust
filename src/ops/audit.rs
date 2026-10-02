//! Audit operator implementation
//!
//! Emits the most recent item when a duration ends, then waits for the next
//! item to start a new duration. Built on `throttle` with a trailing edge.

use crate::ops::throttle::{Throttle, ThrottleWhenParam};

/// Emits the latest item when the selector's observable emits.
#[doc(alias = "audit")]
pub type Audit<S, F> = Throttle<S, ThrottleWhenParam<F>>;

/// Emits the latest item after a fixed duration.
#[doc(alias = "auditTime")]
pub type AuditTime<S, D> = Throttle<S, D>;

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::{context::TestCtx, prelude::*, scheduler::test_scheduler::TestScheduler};

  #[rxrust_macro::test]
  fn test_audit_time_emits_latest_at_window_end() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();
    let _sub = subject
      .clone()
      .audit_time(Duration::from_millis(100))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    subject.next(1);
    TestScheduler::advance_by(Duration::from_millis(50));
    subject.next(2);
    assert!(result.borrow().is_empty());
    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(*result.borrow(), vec![2]);

    // Silence: nothing more
    TestScheduler::advance_by(Duration::from_millis(200));
    assert_eq!(*result.borrow(), vec![2]);

    // A new item starts a new window
    subject.next(3);
    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*result.borrow(), vec![2, 3]);
  }

  #[rxrust_macro::test]
  fn test_audit_time_completion_waits_for_window() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();
    let _sub = subject
      .clone()
      .audit_time(Duration::from_millis(100))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    subject.next(1);
    subject.clone().complete();

    // Like RxJS, completion waits for the open window to end, then the
    // pending item is emitted and the stream completes.
    assert!(result.borrow().is_empty());
    assert!(!*completed.borrow());
    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*result.borrow(), vec![1]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_audit_with_selector() {
    TestScheduler::init();
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();
    let _sub = subject
      .clone()
      .audit(|_| TestCtx::timer(Duration::from_millis(10)))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    subject.next(1);
    subject.next(2);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(*result.borrow(), vec![2]);
  }
}
