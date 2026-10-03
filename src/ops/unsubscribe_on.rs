//! Delay the upstream cancellation method while closing downstream delivery
//! now.
use crate::{
  context::{CellArc, Context, SharedCell},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskState},
  subscription::Subscription,
};

/// Delays explicit or RAII cancellation using the selected scheduler.
#[derive(Clone)]
pub struct UnsubscribeOn<S, Sch> {
  pub source: S,
  pub delay: Duration,
  pub scheduler: Sch,
}
impl<S: ObservableType, Sch> ObservableType for UnsubscribeOn<S, Sch> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Delivery {
  Active,
  Terminated,
  Cancelled,
}

pub struct UnsubscribeOnObserver<O> {
  observer: O,
  delivery: CellArc<Delivery>,
}
impl<O, I, E> Observer<I, E> for UnsubscribeOnObserver<O>
where
  O: Observer<I, E>,
{
  fn next(&mut self, value: I) {
    if self.delivery.get() == Delivery::Active {
      self.observer.next(value);
    }
  }
  fn error(self, err: E) {
    if self
      .delivery
      .compare_exchange(Delivery::Active, Delivery::Terminated)
      .is_ok()
    {
      self.observer.error(err);
    }
  }
  fn complete(self) {
    if self
      .delivery
      .compare_exchange(Delivery::Active, Delivery::Terminated)
      .is_ok()
    {
      self.observer.complete();
    }
  }
  fn is_closed(&self) -> bool {
    self.delivery.get() != Delivery::Active || self.observer.is_closed()
  }
}

/// Ordinary Drop does not cancel. Explicit and RAII cancellation both close
/// delivery immediately and transfer upstream ownership to one scheduled task.
pub struct UnsubscribeOnSubscription<U, Sch> {
  upstream: U,
  delivery: CellArc<Delivery>,
  delay: Duration,
  scheduler: Sch,
}
impl<U, Sch> Subscription for UnsubscribeOnSubscription<U, Sch>
where
  U: Subscription,
  Sch: Scheduler<Task<Option<U>>>,
{
  fn unsubscribe(self) {
    if self
      .delivery
      .compare_exchange(Delivery::Active, Delivery::Cancelled)
      .is_ok()
    {
      let task = Task::new(Some(self.upstream), |upstream| {
        if let Some(upstream) = upstream.take() {
          upstream.unsubscribe();
        }
        TaskState::Finished
      });
      // Dropping a TaskHandle does not cancel the scheduled operation.
      self.scheduler.schedule(task, Some(self.delay));
    }
  }
  fn is_closed(&self) -> bool {
    self.delivery.get() != Delivery::Active || self.upstream.is_closed()
  }
}
impl<S, C, Sch> CoreObservable<C> for UnsubscribeOn<S, Sch>
where
  C: Context,
  S: CoreObservable<C::With<UnsubscribeOnObserver<C::Inner>>>,
  Sch: Scheduler<Task<Option<S::Unsub>>>,
{
  type Unsub = UnsubscribeOnSubscription<S::Unsub, Sch>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let delivery = CellArc::from(Delivery::Active);
    let upstream = self.source.subscribe(
      context.transform(|observer| UnsubscribeOnObserver { observer, delivery: delivery.clone() }),
    );
    UnsubscribeOnSubscription { upstream, delivery, delay: self.delay, scheduler: self.scheduler }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
  };

  use crate::{
    context::{LocalCtx, TestCtx},
    prelude::*,
    scheduler::{Scheduler, Task, TaskHandle},
    subscription::ClosureSubscription,
    test_support::Manual,
  };

  #[rxrust_macro::test]
  fn deadline_starts_at_request_and_delivery_stops_immediately() {
    TestScheduler::init();
    let source = Manual::default();
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let sub = TestCtx::new(source.clone())
      .unsubscribe_on(Duration::from_millis(10))
      .on_error(|_| {})
      .subscribe(move |x| v.borrow_mut().push(x));
    source.next(0, 1);
    TestScheduler::advance_by(Duration::from_millis(7));
    sub.unsubscribe();
    source.next(0, 2);
    assert_eq!(*values.borrow(), vec![1]);
    TestScheduler::advance_by(Duration::from_millis(9));
    assert_eq!(source.cancellations(0), 0);
    TestScheduler::advance_by(Duration::from_millis(1));
    assert_eq!(source.cancellations(0), 1);
    TestScheduler::flush();
    assert_eq!(source.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn zero_delay_is_scheduled_and_raii_uses_same_path() {
    TestScheduler::init();
    let source = Manual::default();
    {
      let _guard = TestCtx::new(source.clone())
        .unsubscribe_on(Duration::ZERO)
        .on_error(|_| {})
        .subscribe(|_| {})
        .unsubscribe_when_dropped();
    }
    assert_eq!(TestScheduler::pending_count(), 1);
    assert_eq!(source.cancellations(0), 0);
    TestScheduler::advance_by(Duration::ZERO);
    assert_eq!(source.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn natural_completion_error_and_plain_drop_do_not_schedule_cancellation() {
    TestScheduler::init();
    let source = Manual::default();
    let complete = TestCtx::new(source.clone())
      .unsubscribe_on(Duration::ZERO)
      .on_error(|_| {})
      .subscribe(|_| {});
    source.complete(0);
    assert!(complete.is_closed());
    complete.unsubscribe();
    let error = TestCtx::new(source.clone())
      .unsubscribe_on(Duration::ZERO)
      .on_error(|_| {})
      .subscribe(|_| {});
    source.error(1);
    assert!(error.is_closed());
    error.unsubscribe();
    let values = Rc::new(Cell::new(0));
    let v = values.clone();
    let dropped = TestCtx::new(source.clone())
      .unsubscribe_on(Duration::ZERO)
      .on_error(|_| {})
      .subscribe(move |_| v.set(v.get() + 1));
    drop(dropped);
    source.next(2, 1);
    assert_eq!(values.get(), 1);
    assert_eq!(TestScheduler::pending_count(), 0);
    for index in 0..3 {
      assert_eq!(source.cancellations(index), 0);
    }
  }

  #[rxrust_macro::test]
  fn synchronous_take_cancellation_waits_for_installation() {
    TestScheduler::init();
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let sub = TestCtx::create(move |e| {
      e.next(1);
      ClosureSubscription(move || c.set(c.get() + 1))
    })
    .unsubscribe_on(Duration::from_millis(10))
    .take(1)
    .subscribe(|_| {});
    sub.unsubscribe();
    assert_eq!(TestScheduler::pending_count(), 1);
    assert_eq!(calls.get(), 0);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(calls.get(), 1);
  }

  #[rxrust_macro::test]
  fn nested_delays_start_when_cancellation_reaches_each_operator() {
    TestScheduler::init();
    let source = Manual::default();
    TestCtx::new(source.clone())
      .unsubscribe_on(Duration::from_millis(10))
      .unsubscribe_on(Duration::from_millis(20))
      .on_error(|_| {})
      .subscribe(|_| {})
      .unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(20));
    assert_eq!(source.cancellations(0), 0);
    TestScheduler::advance_by(Duration::from_millis(9));
    assert_eq!(source.cancellations(0), 0);
    TestScheduler::advance_by(Duration::from_millis(1));
    assert_eq!(source.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn late_terminal_notifications_do_not_replace_requested_cancellation() {
    TestScheduler::init();
    let source = Manual::default();
    let terminals = Rc::new(Cell::new(0));
    for index in 0..2 {
      let complete = terminals.clone();
      let error = terminals.clone();
      TestCtx::new(source.clone())
        .unsubscribe_on(Duration::from_millis(10))
        .on_complete(move || complete.set(complete.get() + 1))
        .on_error(move |_| error.set(error.get() + 1))
        .subscribe(|_| {})
        .unsubscribe();
      if index == 0 {
        source.complete(index);
      } else {
        source.error(index);
      }
    }
    assert_eq!(terminals.get(), 0);
    assert_eq!(TestScheduler::pending_count(), 2);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(source.cancellations(1), 1);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test(shared)]
  async fn shared_scheduler_can_own_the_upstream_cancellation() {
    let (sent, received) = tokio::sync::oneshot::channel();
    let sub = Shared::create(move |emitter| {
      emitter.next(1);
      ClosureSubscription(move || sent.send(()).unwrap())
    })
    .unsubscribe_on(Duration::ZERO)
    .take(1)
    .subscribe(|_| {});
    sub.unsubscribe();
    tokio::time::timeout(Duration::from_secs(5), received)
      .await
      .unwrap()
      .unwrap();
  }

  #[derive(Clone, Default)]
  struct InstanceScheduler {
    id: usize,
    calls: Rc<RefCell<Vec<usize>>>,
  }
  impl<S: 'static> Scheduler<Task<S>> for InstanceScheduler {
    fn schedule(&self, task: Task<S>, delay: Option<Duration>) -> TaskHandle {
      self.calls.borrow_mut().push(self.id);
      TestScheduler.schedule(task, delay)
    }
  }
  #[rxrust_macro::test]
  fn default_and_explicit_variants_keep_selected_scheduler_instances() {
    TestScheduler::init();
    let source = Manual::default();
    let calls = Rc::new(RefCell::new(vec![]));
    let current = InstanceScheduler { id: 7, calls: calls.clone() };
    let explicit = InstanceScheduler { id: 9, calls: calls.clone() };
    LocalCtx::from_parts(source.clone(), current.clone())
      .unsubscribe_on(Duration::ZERO)
      .on_error(|_| {})
      .subscribe(|_| {})
      .unsubscribe();
    LocalCtx::from_parts(source.clone(), current)
      .unsubscribe_on_with(Duration::ZERO, explicit)
      .on_error(|_| {})
      .subscribe(|_| {})
      .unsubscribe();
    assert_eq!(*calls.borrow(), vec![7, 9]);
    TestScheduler::flush();
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(source.cancellations(1), 1);
  }

  #[rxrust_macro::test]
  fn multisource_short_circuit_preserves_each_cancellation_policy() {
    TestScheduler::init();
    let a = Manual::default();
    let b = Manual::default();
    TestCtx::new(a.clone())
      .unsubscribe_on(Duration::from_millis(10))
      .merge(TestCtx::new(b.clone()))
      .take(1)
      .on_error(|_| {})
      .subscribe(|_| {});
    a.next(0, 1);
    assert_eq!(a.cancellations(0), 0);
    assert_eq!(b.cancellations(0), 1);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(a.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn ref_count_reuses_connection_and_old_timer_only_releases_old_reference() {
    TestScheduler::init();
    let source = Manual::default();
    let shared = TestCtx::new(source.clone())
      .publish()
      .ref_count()
      .unsubscribe_on_with(Duration::from_millis(10), TestScheduler);
    let old = shared.clone().on_error(|_| {}).subscribe(|_| {});
    old.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(5));
    let new = shared.clone().on_error(|_| {}).subscribe(|_| {});
    assert_eq!(source.subscriptions(), 1);
    TestScheduler::advance_by(Duration::from_millis(5));
    assert_eq!(source.cancellations(0), 0);
    new.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(source.cancellations(0), 1);
    let fresh = shared.on_error(|_| {}).subscribe(|_| {});
    assert_eq!(source.subscriptions(), 2);
    fresh.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(source.cancellations(1), 1);
  }

  #[rxrust_macro::test]
  fn interval_advances_through_gap_without_replay_then_restarts_after_disconnect() {
    TestScheduler::init();
    let starts = Rc::new(Cell::new(0));
    let s = starts.clone();
    let ticks = Rc::new(Cell::new(0));
    let t = ticks.clone();
    let shared = TestCtx::defer(move || {
      s.set(s.get() + 1);
      let t = t.clone();
      TestCtx::interval(Duration::from_millis(10)).tap(move |_| t.set(t.get() + 1))
    })
    .publish()
    .ref_count()
    .unsubscribe_on(Duration::from_millis(100));
    let old_values = Rc::new(RefCell::new(vec![]));
    let v = old_values.clone();
    let old = shared
      .clone()
      .subscribe(move |x| v.borrow_mut().push(x));
    TestScheduler::advance_by(Duration::from_millis(25));
    assert_eq!(*old_values.borrow(), vec![0, 1]);
    old.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(30));
    let before_join = ticks.get();
    assert!(before_join >= 5);
    let values = Rc::new(RefCell::new(vec![]));
    let v = values.clone();
    let new = shared
      .clone()
      .subscribe(move |x| v.borrow_mut().push(x));
    assert!(values.borrow().is_empty());
    assert_eq!(starts.get(), 1);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(values.borrow()[0], before_join);
    assert_eq!(*old_values.borrow(), vec![0, 1]);
    TestScheduler::advance_by(Duration::from_millis(60));
    assert_eq!(starts.get(), 1);
    new.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(101));
    let stopped = ticks.get();
    TestScheduler::advance_by(Duration::from_millis(30));
    assert_eq!(ticks.get(), stopped);
    let fresh_values = Rc::new(RefCell::new(vec![]));
    let v = fresh_values.clone();
    let fresh = shared.subscribe(move |x| v.borrow_mut().push(x));
    assert_eq!(starts.get(), 2);
    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(*fresh_values.borrow(), vec![0]);
    fresh.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(101));
  }
}
