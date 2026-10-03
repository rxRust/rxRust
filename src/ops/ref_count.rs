//! Reference-counted connections use Subject's logical subscription count.
//! Source callbacks and cancellation always run outside the connection lock.
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType, connectable::ConnectableObservable},
  observer::Observer,
  subject::Subject,
  subscription::{SingleAssignment, Subscription, single_assignment::State},
};
pub struct RefCount<S, P, C> {
  pub(crate) connectable: ConnectableObservable<S, P>,
  pub(crate) connection: C,
}
impl<S: Clone, P: Clone, C: Clone> Clone for RefCount<S, P, C> {
  fn clone(&self) -> Self {
    Self { connectable: self.connectable.clone(), connection: self.connection.clone() }
  }
}
impl<S, P, C> ObservableType for RefCount<S, P, C>
where
  Subject<P>: ObservableType,
{
  type Item<'a>
    = <Subject<P> as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <Subject<P> as ObservableType>::Err;
}

/// An occupied slot reserves a connection even before subscribe returns.
pub struct SourceConnection<H> {
  handle: H,
  terminated: Arc<AtomicBool>,
}

pub type Connection<C, U> = <C as Context>::RcMut<
  Option<SourceConnection<SingleAssignment<<C as Context>::RcMut<State<U>>>>>,
>;

pub struct ConnectionObserver<P> {
  subject: Subject<P>,
  terminated: Arc<AtomicBool>,
}
impl<I, E, P> Observer<I, E> for ConnectionObserver<P>
where
  Subject<P>: Observer<I, E>,
{
  fn next(&mut self, value: I) { self.subject.next(value); }
  fn error(self, err: E) {
    self.terminated.store(true, Ordering::SeqCst);
    self.subject.error(err);
  }
  fn complete(self) {
    self.terminated.store(true, Ordering::SeqCst);
    self.subject.complete();
  }
  fn is_closed(&self) -> bool { self.subject.is_closed() }
}
impl<Ctx, S, P, C, U, Q> CoreObservable<Ctx> for RefCount<S, P, C>
where
  Ctx: Context,
  P: Clone,
  C: RcDerefMut<Target = Option<SourceConnection<SingleAssignment<Q>>>>,
  Q: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
  S: Clone + CoreObservable<Ctx::With<ConnectionObserver<P>>, Unsub = U>,
  Subject<P>: CoreObservable<Ctx>,
{
  type Unsub = RefCountSubscription<<Subject<P> as CoreObservable<Ctx>>::Unsub, P, C>;
  fn subscribe(self, context: Ctx) -> Self::Unsub {
    let scheduler = context.scheduler().clone();
    let subject = self.connectable.subject;
    let inner = subject.clone().subscribe(context);
    let (start, previous) = {
      let mut connection = self.connection.rc_deref_mut();
      if !subject.is_empty()
        && connection
          .as_ref()
          .is_none_or(|c| c.terminated.load(Ordering::SeqCst))
      {
        let handle = SingleAssignment::<Q>::new();
        let terminated = Arc::new(AtomicBool::new(false));
        let previous = connection
          .replace(SourceConnection { handle: handle.clone(), terminated: terminated.clone() });
        (Some((handle, terminated)), previous)
      } else {
        (None, None)
      }
    };
    // Even dropping a naturally terminated source's handle can run user code.
    drop(previous);
    if let Some((handle, terminated)) = start {
      let observer = ConnectionObserver { subject: subject.clone(), terminated };
      handle.set(
        self
          .connectable
          .source
          .subscribe(Ctx::With::from_parts(observer, scheduler)),
      );
    }
    RefCountSubscription { inner, subject, connection: self.connection }
  }
}
pub struct RefCountSubscription<U, P, C> {
  inner: U,
  subject: Subject<P>,
  connection: C,
}
impl<U, P, C, H> Subscription for RefCountSubscription<U, P, C>
where
  U: Subscription,
  C: RcDerefMut<Target = Option<SourceConnection<H>>>,
  H: Subscription,
{
  fn unsubscribe(self) {
    self.inner.unsubscribe();
    let connection = {
      let mut connection = self.connection.rc_deref_mut();
      if self.subject.is_empty() { connection.take() } else { None }
    };
    if let Some(connection) = connection
      && !connection.terminated.load(Ordering::SeqCst)
    {
      connection.handle.unsubscribe();
    }
  }
  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

#[cfg(test)]
mod tests {
  #[rxrust_macro::test]
  fn cancellation_can_reconnect_without_reusing_the_detached_handle() {
    use crate::subscription::{BoxedSubscription, ClosureSubscription};
    let on_cancel = Rc::new(RefCell::new(None::<Box<dyn FnOnce()>>));
    let starts = Rc::new(std::cell::Cell::new(0));
    let stops = Rc::new(std::cell::Cell::new(0));
    let callback = on_cancel.clone();
    let subscribed = starts.clone();
    let cancelled = stops.clone();
    let shared = Local::create::<i32, std::convert::Infallible, _, _>(move |_| {
      subscribed.set(subscribed.get() + 1);
      let callback = callback.clone();
      let cancelled = cancelled.clone();
      ClosureSubscription(move || {
        cancelled.set(cancelled.get() + 1);
        let callback = callback.borrow_mut().take();
        if let Some(callback) = callback {
          callback();
        }
      })
    })
    .publish()
    .ref_count();
    let old = shared.clone().subscribe(|_| {});
    let holder = Rc::new(RefCell::new(None));
    let new = holder.clone();
    *on_cancel.borrow_mut() = Some(Box::new(move || {
      *new.borrow_mut() = Some(BoxedSubscription::new(shared.subscribe(|_| {})));
    }));
    old.unsubscribe();
    assert_eq!(starts.get(), 2);
    assert_eq!(stops.get(), 1);
    holder.borrow_mut().take().unwrap().unsubscribe();
    assert_eq!(stops.get(), 2);
  }

  #[rxrust_macro::test]
  fn synchronous_termination_does_not_cancel_late_handle() {
    use crate::subscription::ClosureSubscription;
    let starts = Rc::new(std::cell::Cell::new(0));
    let stops = Rc::new(std::cell::Cell::new(0));
    let subscribed = starts.clone();
    let cancelled = stops.clone();
    let shared = Local::create::<i32, std::convert::Infallible, _, _>(move |observer| {
      subscribed.set(subscribed.get() + 1);
      observer.complete();
      let cancelled = cancelled.clone();
      ClosureSubscription(move || cancelled.set(cancelled.get() + 1))
    })
    .publish()
    .ref_count();
    let first = shared.clone().subscribe(|_| {});
    let second = shared.subscribe(|_| {});
    assert!(first.is_closed());
    assert!(second.is_closed());
    first.unsubscribe();
    second.unsubscribe();
    assert_eq!(starts.get(), 2);
    assert_eq!(stops.get(), 0);
  }

  #[rxrust_macro::test]
  fn in_flight_values_follow_subject_membership() {
    use crate::test_support::Manual;
    let source = Manual::default();
    let shared = Local::new(source.clone()).publish().ref_count();
    shared
      .clone()
      .on_error(|_| {})
      .subscribe(|_| {})
      .unsubscribe();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let new = shared
      .on_error(|_| {})
      .subscribe(move |x| v.borrow_mut().push(x));
    source.next(0, 99);
    source.next(1, 2);
    assert_eq!(*values.borrow(), vec![99, 2]);
    new.unsubscribe();
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(source.cancellations(1), 1);
  }

  #[rxrust_macro::test]
  fn natural_termination_and_old_handles_do_not_cancel_new_connection() {
    use crate::test_support::Manual;
    for error in [false, true] {
      let source = Manual::default();
      let shared = Local::new(source.clone()).publish().ref_count();
      let old = shared.clone().on_error(|_| {}).subscribe(|_| {});
      if error {
        source.error(0);
      } else {
        source.complete(0);
      }
      assert!(old.is_closed());
      let new = shared.on_error(|_| {}).subscribe(|_| {});
      old.unsubscribe();
      assert_eq!(source.subscriptions(), 2);
      assert_eq!(source.cancellations(0), 0);
      assert_eq!(source.cancellations(1), 0);
      new.unsubscribe();
      assert_eq!(source.cancellations(1), 1);
    }
  }

  #[rxrust_macro::test]
  fn callback_cancellation_and_explicitly_scheduled_reconnection() {
    use crate::{context::TestCtx, subscription::BoxedSubscription, test_support::Manual};
    TestScheduler::init();
    let mut source = Manual::default();
    source.initial = Some(1);
    let shared = TestCtx::new(source.clone()).publish().ref_count();
    let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
    let h = holder.clone();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let again = shared.clone();
    let sub = shared.on_error(|_| {}).subscribe(move |x| {
      if x == 2 {
        h.borrow_mut().take().unwrap().unsubscribe();
        let again = again.clone();
        let holder = h.clone();
        let values = v.clone();
        TestScheduler.schedule(
          async move {
            *holder.borrow_mut() = Some(BoxedSubscription::new(
              again
                .on_error(|_| {})
                .subscribe(move |value| values.borrow_mut().push(value)),
            ));
          },
          None,
        );
      }
    });
    *holder.borrow_mut() = Some(BoxedSubscription::new(sub));
    source.next(0, 2);
    assert_eq!(source.subscriptions(), 1);
    assert_eq!(source.cancellations(0), 1);
    TestScheduler::flush();
    assert_eq!(source.subscriptions(), 2);
    assert_eq!(*values.borrow(), vec![1]);
    holder.borrow_mut().take().unwrap().unsubscribe();
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(source.cancellations(1), 1);
  }

  #[rxrust_macro::test]
  #[should_panic(expected = "re-entrant Subject emission")]
  fn synchronous_callback_reconnection_obeys_subject_reentrancy_policy() {
    use crate::{context::TestCtx, subscription::BoxedSubscription, test_support::Manual};
    TestScheduler::init();
    let mut source = Manual::default();
    source.initial = Some(1);
    let shared = TestCtx::new(source.clone()).publish().ref_count();
    let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
    let subscription = holder.clone();
    let again = shared.clone();
    let sub = shared.on_error(|_| {}).subscribe(move |value| {
      if value == 2 {
        subscription
          .borrow_mut()
          .take()
          .unwrap()
          .unsubscribe();
        again.clone().on_error(|_| {}).subscribe(|_| {});
      }
    });
    *holder.borrow_mut() = Some(BoxedSubscription::new(sub));
    source.next(0, 2);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn concurrent_subscribers_reserve_one_connection() {
    use std::sync::{
      Arc, Barrier,
      atomic::{AtomicUsize, Ordering},
      mpsc,
    };

    use crate::subscription::ClosureSubscription;
    let starts = Arc::new(AtomicUsize::new(0));
    let stops = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Barrier::new(2));
    let wait = gate.clone();
    let (started, ready) = mpsc::channel();
    let start = starts.clone();
    let stop = stops.clone();
    let source = Shared::create::<i32, std::convert::Infallible, _, _>(move |_| {
      start.fetch_add(1, Ordering::SeqCst);
      started.send(()).unwrap();
      wait.wait();
      ClosureSubscription(move || {
        stop.fetch_add(1, Ordering::SeqCst);
      })
    })
    .publish()
    .ref_count();
    let first = source.clone();
    let thread = std::thread::spawn(move || first.subscribe(|_| {}));
    ready
      .recv_timeout(std::time::Duration::from_secs(5))
      .unwrap();
    let second = source.subscribe(|_| {});
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    second.unsubscribe();
    gate.wait();
    thread.join().unwrap().unsubscribe();
    assert_eq!(stops.load(Ordering::SeqCst), 1);
  }
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
