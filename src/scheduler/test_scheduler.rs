//! Test Scheduler for deterministic testing of time-based operators.
//!
//! Provides virtual time that only advances when explicitly instructed,
//! enabling deterministic testing of `delay`, `debounce`, `interval`, etc.
//!
//! # Features
//!
//! - **Virtual Time**: Simulated time that only advances when explicitly
//!   instructed
//! - **Zero-Sized Type**: No runtime overhead, uses thread-local storage
//! - **Synchronous Execution**: Tasks execute synchronously when time is
//!   advanced
//! - **Full Compatibility**: Works with all existing time-based operators
//!
//! # Usage
//!
//! ```rust
//! use rxrust::scheduler::TestScheduler;
//! use rxrust::context::TestCtx;
//!
//! // Initialize the test scheduler (required before use)
//! TestScheduler::init();
//!
//! // Create observables using TestCtx
//! TestCtx::of(42).delay(Duration::from_millis(100)).subscribe(|v| ...);
//!
//! // Advance virtual time to trigger delayed emission
//! TestScheduler::advance_by(Duration::from_millis(100));
//!
//! // Or execute all pending tasks
//! TestScheduler::flush();
//! ```
//!
//! # Thread Safety
//!
//! TestScheduler uses thread-local storage, so each thread has its own
//! independent virtual time and task queue. This ensures test isolation when
//! running tests in parallel across different threads.

use std::{
  cell::{Cell, RefCell},
  cmp::Ordering,
  collections::BinaryHeap,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::Poll,
};

use super::{Duration, Schedulable, Scheduler, Task, TaskHandle, TaskState};
use crate::subscription::Subscription;

// ==================== Internal State ====================

struct TestSchedulerState {
  virtual_time: Duration,
  task_queue: BinaryHeap<ScheduledTask>,
  next_task_id: usize,
  initialized: bool,
}

impl Default for TestSchedulerState {
  fn default() -> Self {
    Self {
      virtual_time: Duration::ZERO,
      task_queue: BinaryHeap::new(),
      next_task_id: 0,
      initialized: false,
    }
  }
}

struct ScheduledTask {
  scheduled_time: Duration,
  task_id: usize,
  task: Box<dyn FnMut() -> TaskState>,
  cancelled: Rc<Cell<bool>>,
  handle: TaskHandle,
}

impl PartialEq for ScheduledTask {
  fn eq(&self, other: &Self) -> bool {
    self.scheduled_time == other.scheduled_time && self.task_id == other.task_id
  }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for ScheduledTask {
  fn cmp(&self, other: &Self) -> Ordering {
    // Min-heap: earlier times first, then FIFO by task_id
    other
      .scheduled_time
      .cmp(&self.scheduled_time)
      .then_with(|| other.task_id.cmp(&self.task_id))
  }
}

thread_local! {
  static TEST_SCHEDULER_STATE: RefCell<TestSchedulerState>
    = RefCell::new(TestSchedulerState::default());
  /// Used by TestTaskFuture to communicate sleep duration back to the scheduler
  static PENDING_SLEEP: Cell<Option<Duration>> = const { Cell::new(None) };
}

// ==================== TestScheduler ====================

/// A virtual time scheduler for deterministic testing.
///
/// This is a zero-sized type that accesses thread-local state.
/// All instances in the same thread share the same virtual time and task queue.
#[derive(Clone, Copy, Default)]
pub struct TestScheduler;

// ==================== Task Future Wrapper ====================

pub struct TestTaskFuture<S> {
  task: Task<S>,
}

impl<S> Future for TestTaskFuture<S> {
  type Output = ();

  fn poll(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    match this.task.step() {
      TaskState::Finished => Poll::Ready(()),
      TaskState::Yield => Poll::Pending,
      TaskState::Sleeping(duration) => {
        // Store the sleep duration in thread-local for the scheduler to pick up
        PENDING_SLEEP.with(|cell| cell.set(Some(duration)));
        Poll::Pending
      }
    }
  }
}

impl<S: 'static> Schedulable<TestScheduler> for Task<S> {
  type Future = TestTaskFuture<S>;

  fn into_future(self, _scheduler: &TestScheduler) -> Self::Future { TestTaskFuture { task: self } }
}

// ==================== TestScheduler Implementation ====================

impl TestScheduler {
  /// Initialize or reset the test scheduler state.
  ///
  /// This method must be called at the start of each test to ensure clean
  /// state. It resets the virtual time to zero, clears the task queue, and
  /// resets the task ID counter.
  ///
  /// # Panics
  ///
  /// Other methods will panic if `init()` has not been called first.
  pub fn init() {
    TEST_SCHEDULER_STATE.with(|state| {
      let mut state = state.borrow_mut();
      state.virtual_time = Duration::ZERO;
      state.task_queue.clear();
      state.next_task_id = 0;
      state.initialized = true;
    });
  }

  fn ensure_initialized() {
    TEST_SCHEDULER_STATE.with(|state| {
      assert!(
        state.borrow().initialized,
        "TestScheduler::init() must be called before using the scheduler"
      );
    });
  }

  /// Get the current virtual time.
  ///
  /// Virtual time only advances when explicitly instructed via `advance_by()`
  /// or `flush()`.
  ///
  /// # Panics
  ///
  /// Panics if `init()` has not been called first.
  pub fn now() -> Duration {
    Self::ensure_initialized();
    TEST_SCHEDULER_STATE.with(|state| state.borrow().virtual_time)
  }

  /// Get the number of pending tasks in the queue.
  ///
  /// # Panics
  ///
  /// Panics if `init()` has not been called first.
  pub fn pending_count() -> usize {
    Self::ensure_initialized();
    TEST_SCHEDULER_STATE.with(|state| state.borrow().task_queue.len())
  }

  /// Check if there are no pending tasks.
  ///
  /// # Panics
  ///
  /// Panics if `init()` has not been called first.
  pub fn is_empty() -> bool {
    Self::ensure_initialized();
    TEST_SCHEDULER_STATE.with(|state| state.borrow().task_queue.is_empty())
  }

  fn execute_tasks_until(target_time: Option<Duration>) {
    loop {
      let task = TEST_SCHEDULER_STATE.with(|state| {
        let mut state = state.borrow_mut();

        // Check if we should stop (no tasks or past target time)
        let should_stop = state
          .task_queue
          .peek()
          .is_none_or(|peek| target_time.is_some_and(|limit| peek.scheduled_time > limit));
        if should_stop {
          return None;
        }

        let scheduled_task = state.task_queue.pop().unwrap();
        state.virtual_time = scheduled_task.scheduled_time;
        Some(scheduled_task)
      });

      let Some(mut scheduled_task) = task else {
        break;
      };

      let result = (scheduled_task.task)();

      TEST_SCHEDULER_STATE.with(|state| {
        let mut state = state.borrow_mut();
        match result {
          TaskState::Finished => {
            scheduled_task.handle.mark_finished();
          }
          TaskState::Yield => {
            Self::reschedule_task(&mut state, scheduled_task, Duration::ZERO);
          }
          TaskState::Sleeping(sleep_duration) => {
            Self::reschedule_task(&mut state, scheduled_task, sleep_duration);
          }
        }
      });
    }
  }

  fn reschedule_task(
    state: &mut TestSchedulerState, scheduled_task: ScheduledTask, delay: Duration,
  ) {
    let task_id = state.next_task_id;
    let scheduled_time = state.virtual_time + delay;
    state.next_task_id += 1;
    state.task_queue.push(ScheduledTask {
      scheduled_time,
      task_id,
      task: scheduled_task.task,
      cancelled: scheduled_task.cancelled,
      handle: scheduled_task.handle,
    });
  }

  /// Advance virtual time by the specified duration and execute due tasks.
  ///
  /// Tasks are executed in order of their scheduled time, with FIFO ordering
  /// for tasks scheduled at the same time.
  ///
  /// # Task State Handling
  ///
  /// - `TaskState::Finished`: Task is removed from the queue
  /// - `TaskState::Yield`: Task is rescheduled for immediate execution
  /// - `TaskState::Sleeping(duration)`: Task is rescheduled for `current_time +
  ///   duration`
  ///
  /// # Panics
  ///
  /// Panics if `init()` has not been called first.
  pub fn advance_by(duration: Duration) {
    Self::ensure_initialized();
    let target_time = TEST_SCHEDULER_STATE.with(|state| state.borrow().virtual_time + duration);

    Self::execute_tasks_until(Some(target_time));

    TEST_SCHEDULER_STATE.with(|state| {
      state.borrow_mut().virtual_time = target_time;
    });
  }

  /// Execute all pending tasks by advancing time to each task's scheduled time.
  ///
  /// Tasks that reschedule themselves will continue to be executed until they
  /// return `TaskState::Finished` or are cancelled.
  ///
  /// # Panics
  ///
  /// Panics if `init()` has not been called first.
  pub fn flush() {
    Self::ensure_initialized();
    Self::execute_tasks_until(None);
  }
}

impl<S> Scheduler<S> for TestScheduler
where
  S: Schedulable<Self> + 'static,
  S::Future: 'static,
{
  fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
    TestScheduler::ensure_initialized();
    let mut future = source.into_future(self);

    TEST_SCHEDULER_STATE.with(|state| {
      let mut state = state.borrow_mut();

      let scheduled_time = state.virtual_time + delay.unwrap_or(Duration::ZERO);
      let handle = TaskHandle::new();
      let task_id = state.next_task_id;
      state.next_task_id += 1;

      let handle_clone = handle.clone();

      let task_closure = Box::new(move || -> TaskState {
        if handle_clone.is_closed() {
          return TaskState::Finished;
        }

        // Clear any previous pending sleep before polling
        PENDING_SLEEP.with(|cell| cell.set(None));

        let waker = futures::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);

        // SAFETY: We never move the future after pinning it here
        let pinned = unsafe { Pin::new_unchecked(&mut future) };
        match pinned.poll(&mut cx) {
          Poll::Ready(()) => TaskState::Finished,
          Poll::Pending => {
            // Check if the future requested a sleep duration via thread-local
            // This is set by TestTaskFuture when Task returns TaskState::Sleeping
            PENDING_SLEEP.with(|cell| {
              cell
                .take()
                .map_or(TaskState::Yield, TaskState::Sleeping)
            })
          }
        }
      });

      state.task_queue.push(ScheduledTask {
        scheduled_time,
        task_id,
        task: task_closure,
        cancelled: Rc::new(Cell::new(false)),
        handle: handle.clone(),
      });

      handle
    })
  }
}

#[cfg(test)]
mod tests {
  use std::mem;

  use super::*;

  // ==================== Basic Properties ====================

  #[rxrust_macro::test]
  fn test_zero_sized_and_copy() {
    assert_eq!(mem::size_of::<TestScheduler>(), 0);

    let s1 = TestScheduler;
    let s2 = s1;
    let _s3 = s1; // Copy works
    let _s4 = s2;
  }

  #[rxrust_macro::test]
  fn test_init_and_reset() {
    TestScheduler::init();
    assert_eq!(TestScheduler::now(), Duration::ZERO);
    assert!(TestScheduler::is_empty());

    // Modify state
    TEST_SCHEDULER_STATE.with(|s| {
      s.borrow_mut().virtual_time = Duration::from_millis(100);
    });
    assert_eq!(TestScheduler::now(), Duration::from_millis(100));

    // Re-init resets
    TestScheduler::init();
    assert_eq!(TestScheduler::now(), Duration::ZERO);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[rxrust_macro::test]
  #[should_panic(expected = "TestScheduler::init() must be called")]
  fn test_panics_without_init() {
    TEST_SCHEDULER_STATE.with(|s| s.borrow_mut().initialized = false);
    TestScheduler::now();
  }

  // ==================== Time Advancement ====================

  #[rxrust_macro::test]
  fn test_advance_by_cumulative() {
    TestScheduler::init();

    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(TestScheduler::now(), Duration::from_millis(100));

    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(TestScheduler::now(), Duration::from_millis(150));
  }

  #[rxrust_macro::test]
  fn test_flush_empty_queue() {
    TestScheduler::init();
    let t = TestScheduler::now();
    TestScheduler::flush();
    assert_eq!(TestScheduler::now(), t);
  }

  // ==================== Task Scheduling ====================

  #[rxrust_macro::test]
  fn test_schedule_immediate_and_delayed() {
    TestScheduler::init();

    let results = Rc::new(RefCell::new(Vec::new()));

    // Immediate task
    let r = results.clone();
    TestScheduler.schedule(
      Task::new(r, |v| {
        v.borrow_mut().push("immediate");
        TaskState::Finished
      }),
      None,
    );

    // Delayed task
    let r = results.clone();
    TestScheduler.schedule(
      Task::new(r, |v| {
        v.borrow_mut().push("delayed");
        TaskState::Finished
      }),
      Some(Duration::from_millis(100)),
    );

    assert_eq!(TestScheduler::pending_count(), 2);

    TestScheduler::advance_by(Duration::ZERO);
    assert_eq!(*results.borrow(), vec!["immediate"]);

    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*results.borrow(), vec!["immediate", "delayed"]);
  }

  #[rxrust_macro::test]
  fn test_task_cancellation() {
    TestScheduler::init();

    let executed = Rc::new(Cell::new(false));
    let e = executed.clone();

    let handle = TestScheduler.schedule(
      Task::new(e, |v| {
        v.set(true);
        TaskState::Finished
      }),
      Some(Duration::from_millis(100)),
    );

    handle.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(150));

    assert!(!executed.get());
  }

  #[rxrust_macro::test]
  fn test_fifo_ordering_same_time() {
    TestScheduler::init();

    let order = Rc::new(RefCell::new(Vec::new()));

    for i in 0..5 {
      let o = order.clone();
      TestScheduler.schedule(
        Task::new((o, i), |(v, id)| {
          v.borrow_mut().push(*id);
          TaskState::Finished
        }),
        Some(Duration::from_millis(100)),
      );
    }

    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*order.borrow(), vec![0, 1, 2, 3, 4]);
  }

  // ==================== TaskState Handling ====================

  #[rxrust_macro::test]
  fn test_task_state_yield_and_finish() {
    TestScheduler::init();

    let count = Rc::new(Cell::new(0));
    let c = count.clone();

    TestScheduler.schedule(
      Task::new(c, |v| {
        v.set(v.get() + 1);
        if v.get() < 3 { TaskState::Yield } else { TaskState::Finished }
      }),
      None,
    );

    TestScheduler::flush();
    assert_eq!(count.get(), 3);
    assert!(TestScheduler::is_empty());
  }

  #[rxrust_macro::test]
  fn test_task_rescheduled_at_target_time() {
    TestScheduler::init();

    let count = Rc::new(Cell::new(0));
    let c = count.clone();

    // Schedule a task that will reschedule itself at target_time
    TestScheduler.schedule(
      Task::new(c, |v| {
        v.set(v.get() + 1);
        if v.get() < 3 { TaskState::Yield } else { TaskState::Finished }
      }),
      Some(Duration::from_millis(100)),
    );

    // Advance to exactly 100ms - task should execute and reschedule
    TestScheduler::advance_by(Duration::from_millis(100));
    // Task should have executed once and rescheduled for immediate execution (Yield
    // with 0 delay) But since we're at target_time, the rescheduled task should
    // also execute
    assert_eq!(count.get(), 3);
    assert!(TestScheduler::is_empty());
  }

  #[rxrust_macro::test]
  fn test_tasks_beyond_target_time_not_discarded() {
    TestScheduler::init();

    let executed = Rc::new(RefCell::new(Vec::new()));

    // Schedule tasks at 50ms and 150ms
    let e = executed.clone();
    TestScheduler.schedule(
      Task::new(e, |v| {
        v.borrow_mut().push(50);
        TaskState::Finished
      }),
      Some(Duration::from_millis(50)),
    );

    let e = executed.clone();
    TestScheduler.schedule(
      Task::new(e, |v| {
        v.borrow_mut().push(150);
        TaskState::Finished
      }),
      Some(Duration::from_millis(150)),
    );

    // Advance by 100ms - only 50ms task should execute
    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*executed.borrow(), vec![50]);
    assert_eq!(TestScheduler::pending_count(), 1); // 150ms task still in queue

    // Advance by another 50ms - 150ms task should now execute
    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(*executed.borrow(), vec![50, 150]);
    assert!(TestScheduler::is_empty());
  }

  // ==================== Future Scheduling ====================

  #[rxrust_macro::test]
  fn test_schedule_future() {
    TestScheduler::init();

    let executed = Rc::new(Cell::new(false));
    let e = executed.clone();

    TestScheduler.schedule(async move { e.set(true) }, Some(Duration::from_millis(100)));

    assert!(!executed.get());
    TestScheduler::advance_by(Duration::from_millis(100));
    assert!(executed.get());
  }

  #[rxrust_macro::test]
  fn test_mixed_tasks_and_futures_fifo() {
    TestScheduler::init();

    let order = Rc::new(RefCell::new(Vec::new()));

    let o = order.clone();
    TestScheduler.schedule(
      Task::new(o, |v| {
        v.borrow_mut().push("task");
        TaskState::Finished
      }),
      None,
    );

    let o = order.clone();
    TestScheduler.schedule(async move { o.borrow_mut().push("future") }, None);

    TestScheduler::flush();
    assert_eq!(*order.borrow(), vec!["task", "future"]);
  }

  // ==================== Shared State ====================

  #[rxrust_macro::test]
  fn test_shared_state_across_instances() {
    TestScheduler::init();

    let s1 = TestScheduler;
    let s2 = TestScheduler;

    s1.schedule(Task::new((), |_| TaskState::Finished), Some(Duration::from_millis(100)));
    assert_eq!(TestScheduler::pending_count(), 1);

    s2.schedule(Task::new((), |_| TaskState::Finished), Some(Duration::from_millis(50)));
    assert_eq!(TestScheduler::pending_count(), 2);
  }
}

#[cfg(test)]
mod integration_tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;
  use crate::{
    context::TestCtx, factory::ObservableFactory, observable::Observable, observer::Observer,
  };

  // ==================== Delay Operator ====================

  #[rxrust_macro::test]
  fn test_delay_basic() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    TestCtx::of(42)
      .delay(Duration::from_millis(100))
      .subscribe(move |v| {
        r.borrow_mut().push(v);
      });

    assert!(received.borrow().is_empty());

    TestScheduler::advance_by(Duration::from_millis(50));
    assert!(received.borrow().is_empty());

    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(*received.borrow(), vec![42]);
  }

  #[rxrust_macro::test]
  fn test_delay_multiple_values() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    TestCtx::from_iter(vec![1, 2, 3])
      .delay(Duration::from_millis(50))
      .subscribe(move |v| r.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(*received.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_delay_cancellation() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    let sub = TestCtx::of(42)
      .delay(Duration::from_millis(100))
      .subscribe(move |v| r.borrow_mut().push(v));

    sub.unsubscribe();
    TestScheduler::advance_by(Duration::from_millis(150));

    assert!(received.borrow().is_empty());
  }

  // ==================== Timer Operator ====================

  #[rxrust_macro::test]
  fn test_timer() {
    TestScheduler::init();

    let fired = Rc::new(Cell::new(false));
    let f = fired.clone();

    TestCtx::timer(Duration::from_millis(75)).subscribe(move |_| f.set(true));

    TestScheduler::advance_by(Duration::from_millis(50));
    assert!(!fired.get());

    TestScheduler::advance_by(Duration::from_millis(30));
    assert!(fired.get());
  }

  // ==================== Debounce Operator ====================

  #[rxrust_macro::test]
  fn test_debounce_basic() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .debounce(Duration::from_millis(50))
      .subscribe(move |v| {
        r.borrow_mut().push(v);
      });

    subject.next(1);
    assert!(received.borrow().is_empty());

    TestScheduler::advance_by(Duration::from_millis(50));
    assert_eq!(*received.borrow(), vec![1]);
  }

  #[rxrust_macro::test]
  fn test_debounce_rapid_emissions() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .debounce(Duration::from_millis(100))
      .subscribe(move |v| {
        r.borrow_mut().push(v);
      });

    // Rapid emissions within debounce window
    subject.next(1);
    TestScheduler::advance_by(Duration::from_millis(30));
    subject.next(2);
    TestScheduler::advance_by(Duration::from_millis(30));
    subject.next(3);

    assert!(received.borrow().is_empty());

    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*received.borrow(), vec![3]); // Only last value
  }

  #[rxrust_macro::test]
  fn test_debounce_spaced_emissions() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .debounce(Duration::from_millis(50))
      .subscribe(move |v| {
        r.borrow_mut().push(v);
      });

    subject.next(1);
    TestScheduler::advance_by(Duration::from_millis(60));

    subject.next(2);
    TestScheduler::advance_by(Duration::from_millis(60));

    assert_eq!(*received.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_debounce_trailing_on_complete() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    let mut subject = TestCtx::subject::<i32, std::convert::Infallible>();
    subject
      .clone()
      .debounce(Duration::from_millis(100))
      .subscribe(move |v| {
        r.borrow_mut().push(v);
      });

    subject.next(42);
    subject.complete();

    // Trailing value emitted synchronously on complete
    assert_eq!(*received.borrow(), vec![42]);
  }

  // ==================== Multiple Delayed Emissions ====================

  #[rxrust_macro::test]
  fn test_multiple_delays_ordering() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));

    // Schedule out of order
    for (delay, val) in [(300, "c"), (100, "a"), (200, "b")] {
      let r = received.clone();
      TestCtx::of(val)
        .delay(Duration::from_millis(delay))
        .subscribe(move |v| r.borrow_mut().push(v));
    }

    TestScheduler::flush();
    assert_eq!(*received.borrow(), vec!["a", "b", "c"]); // Time order
  }

  // ==================== Combined Operators ====================

  #[rxrust_macro::test]
  fn test_delay_with_map() {
    TestScheduler::init();

    let received = Rc::new(RefCell::new(Vec::new()));
    let r = received.clone();

    TestCtx::of(5)
      .map(|x| x * 2)
      .delay(Duration::from_millis(100))
      .map(|x| x + 1)
      .subscribe(move |v| r.borrow_mut().push(v));

    TestScheduler::advance_by(Duration::from_millis(100));
    assert_eq!(*received.borrow(), vec![11]); // (5 * 2) + 1
  }
}
