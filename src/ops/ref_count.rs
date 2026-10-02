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
  context::{Context, RcDerefMut},
  observable::{CoreObservable, Observable, ObservableType, connectable::ConnectableObservable},
  subject::{MulticastSubject, ReplaySubjectOf, Subject, SubjectPtr},
  subscription::Subscription,
};

/// Wraps a `ConnectableObservable` and manages connection based on subscriber
/// count.
///
/// Uses the subject's own subscriber list instead of a separate counter.
pub struct RefCount<S, Sub, ConnPtr> {
  pub(crate) connectable: ConnectableObservable<S, Sub>,
  pub(crate) connection: ConnPtr,
}

impl<S: Clone, Sub: Clone, ConnPtr: Clone> Clone for RefCount<S, Sub, ConnPtr> {
  fn clone(&self) -> Self {
    Self { connectable: self.connectable.clone(), connection: self.connection.clone() }
  }
}

impl<S, Sub, ConnPtr> ObservableType for RefCount<S, Sub, ConnPtr>
where
  Sub: ObservableType,
{
  type Item<'a>
    = Sub::Item<'a>
  where
    Self: 'a;
  type Err = Sub::Err;
}

impl<Ctx, S, Sub, ConnPtr> CoreObservable<Ctx> for RefCount<S, Sub, ConnPtr>
where
  Ctx: Context,
  S: Clone + CoreObservable<Ctx::With<Sub>>,
  Sub: Clone + MulticastSubject + CoreObservable<Ctx>,
  ConnPtr: Clone + RcDerefMut<Target = Option<S::Unsub>> + Subscription,
{
  type Unsub = RefCountSubscription<Sub, <Sub as CoreObservable<Ctx>>::Unsub, ConnPtr>;

  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let subject = self.connectable.fork();
    let inner_sub = subject.clone().subscribe(observer);

    if !subject.is_terminated()
      && subject.subscriber_count() == 1
      && self.connection.rc_deref().is_none()
    {
      *self.connection.rc_deref_mut() = Some(self.connectable.connect::<Ctx>());
    }

    RefCountSubscription { subject, inner: inner_sub, connection: self.connection }
  }
}

/// Subscription for RefCount. Disconnects source when last subscriber leaves.
pub struct RefCountSubscription<Sub, InnerSub, ConnPtr> {
  subject: Sub,
  inner: InnerSub,
  connection: ConnPtr,
}

impl<Sub, InnerSub, ConnPtr> Subscription for RefCountSubscription<Sub, InnerSub, ConnPtr>
where
  Sub: MulticastSubject,
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

/// The plain `Subject` that `publish()` uses for an observable `O`.
pub type PublishSubjectOf<'a, O> =
  Subject<SubjectPtr<'a, O, <O as Observable>::Item<'a>, <O as Observable>::Err>>;

/// Return type of [`Observable::share`].
pub type ShareOf<'a, O> =
  <O as Context>::With<
    RefCount<
      <O as Context>::Inner,
      PublishSubjectOf<'a, O>,
      <O as Context>::RcMut<
        Option<
          <<O as Context>::Inner as CoreObservable<
            <O as Context>::With<PublishSubjectOf<'a, O>>,
          >>::Unsub,
        >,
      >,
    >,
  >;

/// Return type of [`Observable::share_replay`].
pub type ShareReplayOf<'a, O> = <O as Context>::With<
  RefCount<
    <O as Context>::Inner,
    ReplaySubjectOf<'a, O>,
    <O as Context>::RcMut<
      Option<
        <<O as Context>::Inner as CoreObservable<
          <O as Context>::With<ReplaySubjectOf<'a, O>>,
        >>::Unsub,
      >,
    >,
  >,
>;

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

  #[rxrust_macro::test]
  fn test_share_connects_once_for_two_subscribers() {
    let a = Rc::new(RefCell::new(Vec::new()));
    let b = Rc::new(RefCell::new(Vec::new()));
    let a_c = a.clone();
    let b_c = b.clone();

    let mut source = Local::subject::<i32, std::convert::Infallible>();
    let shared = source.clone().share();

    let sub_a = shared
      .clone()
      .subscribe(move |v| a_c.borrow_mut().push(v));
    let sub_b = shared.subscribe(move |v| b_c.borrow_mut().push(v));
    source.next(1);
    source.next(2);

    assert_eq!(*a.borrow(), vec![1, 2]);
    assert_eq!(*b.borrow(), vec![1, 2]);
    // One connection to the source no matter how many subscribers
    assert_eq!(source.inner.subscriber_count(), 1);

    sub_a.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 1);
    sub_b.unsubscribe();
    assert_eq!(source.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_share_replay_late_subscriber_gets_buffer_and_completion() {
    let shared = Local::from_iter(vec![1, 2, 3]).share_replay(2);

    let first = Rc::new(RefCell::new(Vec::new()));
    let first_c = first.clone();
    shared
      .clone()
      .subscribe(move |v| first_c.borrow_mut().push(v));
    assert_eq!(*first.borrow(), vec![1, 2, 3]);

    let late = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let late_c = late.clone();
    let completed_c = completed.clone();
    shared
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| late_c.borrow_mut().push(v));
    assert_eq!(*late.borrow(), vec![2, 3]);
    assert!(*completed.borrow());
  }
}
