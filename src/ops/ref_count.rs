//! RefCount operator - auto-manages ConnectableObservable connection.
//!
//! Connects on first subscription, disconnects when all subscribers leave.
//!
//! # Example
//!
//! ```rust
//! use rxrust::prelude::*;
//!
//! let shared = Local::from_iter([1, 2]).publish().ref_count();
//!
//! let sub = shared.subscribe(|v| println!("Got: {}", v));
//! sub.unsubscribe();
//! ```

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType, connectable::ConnectableObservable},
  subject::{Subject, subscribers::Subscribers},
  subscription::Subscription,
};

/// Wraps a `ConnectableObservable` and manages connection based on subscriber
/// count.
///
/// Uses the Subject's internal subscriber list instead of a separate counter.
pub struct RefCount<S, P, ConnPtr> {
  pub(crate) connectable: ConnectableObservable<S, P>,
  pub(crate) connection: ConnPtr,
}

impl<S: Clone, P: Clone, ConnPtr: Clone> Clone for RefCount<S, P, ConnPtr> {
  fn clone(&self) -> Self {
    Self { connectable: self.connectable.clone(), connection: self.connection.clone() }
  }
}

impl<S, P, ConnPtr> ObservableType for RefCount<S, P, ConnPtr>
where
  Subject<P>: ObservableType,
{
  type Item<'a>
    = <Subject<P> as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <Subject<P> as ObservableType>::Err;
}

impl<Ctx, O, S, P, ConnPtr> CoreObservable<Ctx> for RefCount<S, P, ConnPtr>
where
  Ctx: Context,
  S: Clone + CoreObservable<Ctx::With<Subject<P>>>,
  Subject<P>: CoreObservable<Ctx>,
  P: Clone + RcDeref<Target = Subscribers<O>>,
  ConnPtr: Clone + RcDerefMut<Target = Option<S::Unsub>> + Subscription,
{
  type Unsub = RefCountSubscription<P, <Subject<P> as CoreObservable<Ctx>>::Unsub, ConnPtr>;

  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let subject = self.connectable.fork();
    let inner_sub = subject.clone().subscribe(observer);

    if subject.subscriber_count() == 1 && self.connection.rc_deref().is_none() {
      *self.connection.rc_deref_mut() = Some(self.connectable.connect::<Ctx>());
    }

    RefCountSubscription { subject, inner: inner_sub, connection: self.connection }
  }
}

/// Subscription for RefCount. Disconnects source when last subscriber leaves.
pub struct RefCountSubscription<P, InnerSub, ConnPtr> {
  subject: Subject<P>,
  inner: InnerSub,
  connection: ConnPtr,
}

impl<P, InnerSub, ConnPtr, O> Subscription for RefCountSubscription<P, InnerSub, ConnPtr>
where
  P: RcDeref<Target = Subscribers<O>>,
  InnerSub: Subscription,
  ConnPtr: Subscription,
{
  fn unsubscribe(self) {
    self.inner.unsubscribe();
    if self.subject.is_empty() {
      self.connection.unsubscribe();
    }
  }

  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::{observable::Observable, prelude::*};

  #[rxrust_macro::test]
  fn test_ref_count_basic() {
    let results = Rc::new(RefCell::new(vec![]));
    let r = results.clone();

    let mut source = Local::subject();
    let shared = source.clone().publish().ref_count();

    shared.subscribe(move |v| r.borrow_mut().push(v));
    source.next(42);

    assert_eq!(*results.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_ref_count_multiple_subscribers() {
    let results1 = Rc::new(RefCell::new(vec![]));
    let results2 = Rc::new(RefCell::new(vec![]));

    let mut subject = Local::subject();
    let shared = subject.clone().publish().ref_count();

    let r1 = results1.clone();
    let _sub1 = shared
      .clone()
      .subscribe(move |v| r1.borrow_mut().push(v));

    let r2 = results2.clone();
    let _sub2 = shared.subscribe(move |v| r2.borrow_mut().push(v));

    subject.next(1);
    subject.next(2);

    assert_eq!(*results1.borrow(), vec![1, 2]);
    assert_eq!(*results2.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_ref_count_unsubscribe() {
    let results = Rc::new(RefCell::new(vec![]));

    let mut subject = Local::subject();
    let shared = subject.clone().publish().ref_count();

    let r1 = results.clone();
    let sub1 = shared
      .clone()
      .subscribe(move |v| r1.borrow_mut().push(format!("A:{}", v)));

    subject.next(1);

    let r2 = results.clone();
    let sub2 = shared.subscribe(move |v| r2.borrow_mut().push(format!("B:{}", v)));

    subject.next(2);
    sub1.unsubscribe();
    subject.next(3);
    sub2.unsubscribe();

    let received = results.borrow();
    assert!(received.contains(&"A:1".to_string()));
    assert!(received.contains(&"A:2".to_string()));
    assert!(received.contains(&"B:2".to_string()));
    assert!(received.contains(&"B:3".to_string()));
    assert!(!received.contains(&"A:3".to_string()));
  }
}
