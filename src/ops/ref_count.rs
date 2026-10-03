//! Reference-counted connections. Reference ownership and source connection
//! generations are coordinated independently of Subject's list.
//! Subject registration and emission retain their existing re-entrancy rules.
use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType, connectable::ConnectableObservable},
  observer::Observer,
  subject::{GuardedSubject, Subject},
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
/// State for a connection generation. User callbacks never run under this lock.
pub struct RefCountState<H> {
  references: usize,
  generation: usize,
  connecting: bool,
  terminated: bool,
  connection: Option<H>,
}
impl<H> Default for RefCountState<H> {
  fn default() -> Self {
    Self { references: 0, generation: 0, connecting: false, terminated: false, connection: None }
  }
}
impl<H> RefCountState<H> {
  fn is_active(&self, generation: usize) -> bool {
    self.generation == generation && self.references > 0 && !self.terminated
  }

  fn terminate(&mut self, generation: usize) -> bool {
    if !self.is_active(generation) {
      return false;
    }
    self.terminated = true;
    true
  }
}

pub type Connection<C, U> =
  <C as Context>::RcMut<RefCountState<SingleAssignment<<C as Context>::RcMut<State<U>>>>>;
pub struct ConnectionObserver<P, C> {
  subject: Subject<P>,
  connection: C,
  generation: usize,
}
impl<I, E, P, C, H> Observer<I, E> for ConnectionObserver<P, C>
where
  Subject<P>: GuardedSubject<I, E>,
  C: RcDerefMut<Target = RefCountState<H>>,
{
  fn next(&mut self, v: I) {
    let current = self
      .connection
      .rc_deref()
      .is_active(self.generation);
    if current {
      // Recheck under the Subject broadcast guard: the connection can change
      // after the first check, which also avoids entering Subject for stale
      // input.
      self.subject.next_if(v, || {
        self
          .connection
          .rc_deref()
          .is_active(self.generation)
      });
    }
  }
  fn error(self, e: E) {
    let current = self
      .connection
      .rc_deref()
      .is_active(self.generation);
    if !current {
      return;
    }
    self.subject.error_if(e, || {
      self
        .connection
        .rc_deref_mut()
        .terminate(self.generation)
    });
  }
  fn complete(self) {
    let current = self
      .connection
      .rc_deref()
      .is_active(self.generation);
    if !current {
      return;
    }
    self.subject.complete_if(|| {
      self
        .connection
        .rc_deref_mut()
        .terminate(self.generation)
    });
  }
  fn is_closed(&self) -> bool {
    let current = self
      .connection
      .rc_deref()
      .is_active(self.generation);
    !current || self.subject.is_closed()
  }
}
impl<Ctx, S, P, C, U, Q> CoreObservable<Ctx> for RefCount<S, P, C>
where
  Ctx: Context,
  P: Clone,
  C: RcDerefMut<Target = RefCountState<SingleAssignment<Q>>>,
  Q: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
  S: Clone + CoreObservable<Ctx::With<ConnectionObserver<P, C>>, Unsub = U>,
  Subject<P>: CoreObservable<Ctx>,
{
  type Unsub = RefCountSubscription<<Subject<P> as CoreObservable<Ctx>>::Unsub, C>;
  fn subscribe(self, context: Ctx) -> Self::Unsub {
    let generation = {
      let mut state = self.connection.rc_deref_mut();
      if state.references == 0 {
        state.generation += 1;
        state.connecting = false;
        state.terminated = false;
        state.connection = Some(SingleAssignment::<Q>::new());
      }
      state.references += 1;
      state.generation
    };
    let scheduler = context.scheduler().clone();
    let subject = self.connectable.subject;
    let inner = subject.clone().subscribe(context);
    let slot = {
      let mut state = self.connection.rc_deref_mut();
      if state.generation != generation || state.references == 0 || state.connecting {
        None
      } else {
        state.connecting = true;
        Some(
          state
            .connection
            .as_ref()
            .expect("reserved connection missing")
            .clone(),
        )
      }
    };
    if let Some(slot) = slot {
      let observer =
        ConnectionObserver { subject, connection: self.connection.clone(), generation };
      slot.set(
        self
          .connectable
          .source
          .subscribe(Ctx::With::from_parts(observer, scheduler)),
      );
    }
    RefCountSubscription { inner, connection: self.connection, generation }
  }
}
pub struct RefCountSubscription<U, C> {
  inner: U,
  connection: C,
  generation: usize,
}
impl<U, C, H> Subscription for RefCountSubscription<U, C>
where
  U: Subscription,
  C: RcDerefMut<Target = RefCountState<H>>,
  H: Subscription,
{
  fn unsubscribe(self) {
    self.inner.unsubscribe();
    let (connection, terminated) = {
      let mut state = self.connection.rc_deref_mut();
      if state.generation != self.generation {
        return;
      }
      assert!(state.references > 0, "ref_count reference released twice");
      state.references -= 1;
      if state.references == 0 {
        (state.connection.take(), state.terminated)
      } else {
        (None, false)
      }
    };
    if !terminated {
      connection.unsubscribe();
    }
  }
  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

#[cfg(test)]
mod tests {
  #[rxrust_macro::test]
  fn stale_generation_and_natural_termination_do_not_cancel_new_connection() {
    use crate::test_support::Manual;
    let source = Manual::default();
    let shared = Local::new(source.clone()).publish().ref_count();
    let old = shared.clone().on_error(|_| {}).subscribe(|_| {});
    old.unsubscribe();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let new = shared
      .clone()
      .on_error(|_| {})
      .subscribe(move |x| v.borrow_mut().push(x));
    source.next(0, 99);
    source.complete(0);
    source.next(1, 2);
    assert_eq!(*values.borrow(), vec![2]);
    assert_eq!(source.cancellations(1), 0);
    source.complete(1);
    new.unsubscribe();
    assert_eq!(source.cancellations(1), 0);
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
