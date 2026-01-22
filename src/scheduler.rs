//! # Scheduler System with Unified TaskHandle
//!
//! This module provides a flexible, async-first scheduling system for RxRust
//! observables. It includes the core [`Task`], [`TaskHandle`], and
//! [`Scheduler`] trait, along with default implementations for both
//! single-threaded and multi-threaded execution.
//!
//! ## Core Components
//!
//! - **[`Task<S>`]** - A state machine that bundles data with execution logic
//! - **[`TaskHandle`]** - Unified handle that serves as both [`Subscription`]
//!   and [`Future`]
//! - **[`Scheduler<S>`]** - Trait for executing tasks with optional delays
//! - **[`LocalScheduler`]** - Single-threaded scheduler (no `Send` requirement)
//! - **[`SharedScheduler`]** - Multi-threaded scheduler (requires `Send`)
//!
//! ## Key Design Principles
//!
//! - **Cross-platform async**: Uses tokio for native platforms, web APIs for
//!   WASM
//! - **Panic safety**: Automatically converts panics to task completion via
//!   `CatchUnwind`
//! - **Zero-cost abstractions**: Schedulers are zero-sized types with no
//!   runtime overhead
//! - **Type safety**: Send bounds enforced at compile time through trait
//!   implementations
//! - **Unified interface**: TaskHandle works as both cancellation token and
//!   awaitable future
//! - **Instance-Agnostic Design**: Schedulers are typically zero-sized types or
//!   simple handles. Creating multiple instances of a `Scheduler` (e.g., via
//!   `Default`) does not create new runtimes; they all point to the same global
//!   or thread-local execution environment.
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use std::sync::{Arc, Mutex};
//!
//! use rxrust::scheduler::{LocalScheduler, Scheduler, Task, TaskState};
//!
//! let counter = Arc::new(Mutex::new(0));
//! let counter_clone = counter.clone();
//!
//! let scheduler = LocalScheduler;
//! let task = Task::new(counter_clone, |count| {
//!   *count.lock().unwrap() += 1;
//!   TaskState::Finished
//! });
//!
//! let handle = scheduler.schedule(task, None);
//! // handle.await; // Wait for completion
//! // handle.unsubscribe(); // Or cancel early
//! ```
//!
//! ## Default Implementations
//!
//! Default schedulers (`LocalScheduler`, `SharedScheduler`) are available
//! behind the crate feature flag `scheduler` (enabled by default). Disable
//! default features to avoid pulling in runtime/timer dependencies and provide
//! your own scheduler.
//!
//! ## Custom Scheduler Implementation
//!
//! When implementing custom schedulers, remember that **Schedulers are
//! Handles**. They should not own the runtime itself but rather reference it
//! (e.g., via global state, thread-local storage, or shared handles like
//! `Arc`).
//!
//! This is crucial because rxRust operators frequently instantiate new
//! schedulers via `Default::default()`. If your scheduler creates a new thread
//! pool every time it's instantiated, you will leak resources.
//!
//! 1. **Implement [`Scheduler<S>`]** for your scheduler types
//!
//! ### Example: Custom Scheduler
//!
//! ```rust,no_run
//! use std::future;
//!
//! use rxrust::scheduler::{Duration, Schedulable, Scheduler, SleepProvider, TaskHandle};
//!
//! #[derive(Clone, Copy, Default)]
//! pub struct MyScheduler;
//!
//! // Provide a timer future so rxrust's internal `Task<...>` becomes schedulable automatically.
//! impl SleepProvider for MyScheduler {
//!   type SleepFuture = future::Ready<()>;
//!
//!   fn sleep(&self, _duration: Duration) -> Self::SleepFuture { future::ready(()) }
//! }
//!
//! impl<S> Scheduler<S> for MyScheduler
//! where
//!   S: 'static + Schedulable<MyScheduler>,
//! {
//!   fn schedule(&self, source: S, _delay: Option<Duration>) -> TaskHandle {
//!     let _future = source.into_future(self);
//!     // Custom scheduling logic here.
//!     // Crucially, this should dispatch to a SHARED runtime, not create a new one.
//!     // e.g., runtime::spawn(_future);
//!     TaskHandle::finished()
//!   }
//! }
//! ```
//!
//! [`Subscription`]: crate::subscription::Subscription

// Standard library imports
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Duration, Instant};
use std::{
  any::Any,
  future::Future,
  panic::AssertUnwindSafe,
  pin::Pin,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
  task::{Context, Poll, Waker},
};

// External crate imports
use pin_project_lite::pin_project;
#[cfg(target_arch = "wasm32")]
pub use web_time::{Duration, Instant};

// Internal module imports
use crate::subscription::Subscription;

// ==================== Core Task Types ====================

/// Controls task execution flow and scheduling behavior.
///
/// Returned by task handlers to indicate what the scheduler should do next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
  /// Task completed successfully. The scheduler will drop the task state.
  Finished,
  /// Task yielded control but wants to run again immediately.
  /// Allows cooperative multitasking without blocking the executor.
  Yield,
  /// Task wants to sleep for the specified duration before running again.
  /// Useful for implementing delays, timeouts, or periodic execution.
  Sleeping(Duration),
}

/// A concrete task that bundles state with execution logic.
///
/// Unlike closures, `Task` uses function pointers which enables:
/// - **Type safety**: Send bounds are enforced by the scheduler trait
/// - **Zero allocation**: No heap allocation for the task itself
/// - **State persistence**: State survives across execution steps
/// - **Debuggability**: Function pointers are easier to inspect than closures
///
/// # Example
///
/// ```rust
/// use std::sync::{Arc, Mutex};
///
/// use rxrust::scheduler::{Task, TaskState};
///
/// let counter = Arc::new(Mutex::new(0));
/// let task = Task::new(counter, |count| {
///   *count.lock().unwrap() += 1;
///   if *count.lock().unwrap() >= 10 { TaskState::Finished } else { TaskState::Yield }
/// });
/// ```
pub struct Task<S> {
  /// The state data that persists across task executions
  pub state: S,
  /// Function that executes one step of the task logic
  pub handler: fn(&mut S) -> TaskState,
}

impl<S> Task<S> {
  /// Create a new task with the given state and handler function.
  ///
  /// # Parameters
  ///
  /// - `state`: Initial state data for the task
  /// - `handler`: Function that executes one step of task logic
  ///
  /// # Example
  ///
  /// ```rust
  /// use rxrust::scheduler::{Task, TaskState};
  ///
  /// let task = Task::new(0, |counter| {
  ///   *counter += 1;
  ///   if *counter >= 5 { TaskState::Finished } else { TaskState::Yield }
  /// });
  /// ```
  pub fn new(state: S, handler: fn(&mut S) -> TaskState) -> Self { Self { state, handler } }

  /// Execute a single step of the task, returning the next scheduling action.
  ///
  /// This method calls the task's handler function with mutable access to
  /// the state, allowing the task to update its internal data and decide
  /// what should happen next.
  ///
  /// # Returns
  ///
  /// - [`TaskState::Finished`]: Task completed, scheduler should drop it
  /// - [`TaskState::Yield`]: Task wants to run again immediately
  /// - [`TaskState::Sleeping(duration)`]: Task wants to sleep before next run
  pub fn step(&mut self) -> TaskState { (self.handler)(&mut self.state) }
}

// ==================== Schedulable Trait ====================

/// Types that can be converted into futures by a specific scheduler.
///
/// This trait provides the bridge between different task types and scheduler
/// implementations, allowing schedulers to handle both raw futures and
/// stateful tasks uniformly.
///
/// # Implementations
///
/// - **Futures**: Any `Future<Output = ()>` is directly schedulable
/// - **Tasks**: `Task<S>` requires scheduler-specific conversion to handle
///   state machines with sleep/yield capabilities
///
/// # Design Benefits
///
/// - **Decoupling**: Tasks don't need to know about specific scheduler
///   implementations
/// - **Flexibility**: Schedulers can provide custom future conversion logic
/// - **Uniformity**: Both futures and tasks use the same scheduling interface
pub trait Schedulable<Sch> {
  /// The future type produced by this schedulable item
  type Future: Future<Output = ()>;

  /// Convert this item into a future using the given scheduler's capabilities
  fn into_future(self, scheduler: &Sch) -> Self::Future;
}

/// Blanket implementation: Any `Future<Output = ()>` is directly schedulable.
/// No Scheduler assistance needed - just returns itself.
impl<F, Sch> Schedulable<Sch> for F
where
  F: Future<Output = ()>,
{
  type Future = Self;
  fn into_future(self, _scheduler: &Sch) -> Self::Future { self }
}

// ==================== SleepProvider + TaskFuture ====================

/// Provides a concrete sleep future for a scheduler.
///
/// This is used by [`TaskFuture`] to implement [`TaskState::Sleeping`] without
/// hard-wiring tokio or WASM timers into the task driver.
pub trait SleepProvider: Clone {
  /// The timer future type used for sleeping.
  ///
  /// This future must be owned (must not borrow from `self`) so it can be
  /// stored inside [`TaskFuture`].
  type SleepFuture: Future<Output = ()> + 'static;

  /// Create a sleep future for the given duration.
  fn sleep(&self, duration: Duration) -> Self::SleepFuture;
}

/// A generic driver future for [`Task<S>`] that supports yielding and sleeping.
///
/// This is shared by all schedulers that implement [`SleepProvider`].
pub struct TaskFuture<Sch, S>
where
  Sch: SleepProvider,
{
  scheduler: Sch,
  task: Task<S>,
  pending_sleep: Option<Sch::SleepFuture>,
}

impl<Sch, S> Future for TaskFuture<Sch, S>
where
  Sch: SleepProvider,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
    // Safety: We are manually projecting. `pending_sleep` is structurally pinned.
    let this = unsafe { self.get_unchecked_mut() };

    loop {
      if let Some(ref mut sleep_fut) = this.pending_sleep {
        // Safety: `sleep_fut` is pinned because `self` is pinned.
        let sleep_fut = unsafe { Pin::new_unchecked(sleep_fut) };
        match sleep_fut.poll(ctx) {
          Poll::Ready(()) => this.pending_sleep = None,
          Poll::Pending => return Poll::Pending,
        }
      } else {
        match this.task.step() {
          TaskState::Finished => return Poll::Ready(()),
          TaskState::Yield => {
            ctx.waker().wake_by_ref();
            return Poll::Pending;
          }
          TaskState::Sleeping(dur) => {
            this.pending_sleep = Some(this.scheduler.sleep(dur));
          }
        }
      }
    }
  }
}

impl<Sch, S> Schedulable<Sch> for Task<S>
where
  Sch: SleepProvider + Scheduler<<Sch as SleepProvider>::SleepFuture> + Clone,
  S: 'static,
{
  type Future = TaskFuture<Sch, S>;

  fn into_future(self, scheduler: &Sch) -> Self::Future {
    TaskFuture { scheduler: scheduler.clone(), task: self, pending_sleep: None }
  }
}

// ==================== TaskHandle ====================

/// Internal shared state for coordinating task handles
struct SharedState {
  keep_running: AtomicBool,
  finished: AtomicBool,
  waker: Mutex<Option<Waker>>,
}

/// A unified handle for scheduled tasks that serves dual purposes.
///
/// `TaskHandle` implements both [`Subscription`] and [`Future`], providing:
/// - **Cancellation**: Call `unsubscribe()` to stop the task
/// - **Completion tracking**: Await the handle to wait for task completion
/// - **Status checking**: Use `is_closed()` to check if task is done or
///   cancelled
///
/// # Thread Safety
///
/// TaskHandle is `Clone + Send + Sync`, allowing it to be shared across threads
/// and async contexts safely.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::{Arc, Mutex};
///
/// use rxrust::{
///   scheduler::{LocalScheduler, Scheduler, Task, TaskState},
///   subscription::Subscription,
/// };
///
/// let scheduler = LocalScheduler;
/// let data = Arc::new(Mutex::new(0));
/// let task = Task::new(data, |_| TaskState::Finished);
///
/// let handle = scheduler.schedule(task, None);
///
/// // Option 1: Cancel the task
/// handle.clone().unsubscribe();
///
/// // Option 2: Wait for completion
/// // In async context: handle.await;
///
/// // Option 3: Check status
/// if handle.is_closed() {
///   println!("Task finished or was cancelled");
/// }
/// ```
#[derive(Clone)]
pub struct TaskHandle {
  inner: Arc<SharedState>,
}

impl TaskHandle {
  /// Create a new task handle for an active task.
  ///
  /// This is used internally by schedulers when starting new tasks.
  pub(crate) fn new() -> Self {
    Self {
      inner: Arc::new(SharedState {
        keep_running: AtomicBool::new(true),
        finished: AtomicBool::new(false),
        waker: Mutex::new(None),
      }),
    }
  }

  /// Create a handle for a task that has already completed.
  ///
  /// This is useful for:
  /// - Synchronous operations that complete immediately
  /// - Error cases where no actual scheduling is needed
  /// - Testing scenarios
  ///
  /// The returned handle will immediately return `Poll::Ready(())` when polled
  /// and `is_closed()` will return `true`.
  pub fn finished() -> Self {
    Self {
      inner: Arc::new(SharedState {
        keep_running: AtomicBool::new(false),
        finished: AtomicBool::new(true),
        waker: Mutex::new(None),
      }),
    }
  }

  /// Mark this task handle as finished.
  ///
  /// This is used by schedulers to signal that a task has completed execution.
  /// After calling this method, `is_closed()` will return `true`.
  pub(crate) fn mark_finished(&self) {
    self.inner.finished.store(true, Ordering::Relaxed);
    if let Ok(mut waker) = self.inner.waker.lock()
      && let Some(w) = waker.take()
    {
      w.wake();
    }
  }
}

impl Subscription for TaskHandle {
  fn unsubscribe(self) {
    self
      .inner
      .keep_running
      .store(false, Ordering::Relaxed);
    if let Ok(mut waker) = self.inner.waker.lock()
      && let Some(w) = waker.take()
    {
      w.wake();
    }
  }

  fn is_closed(&self) -> bool {
    !self.inner.keep_running.load(Ordering::Relaxed) || self.inner.finished.load(Ordering::Relaxed)
  }
}

impl Future for TaskHandle {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if self.inner.finished.load(Ordering::Relaxed)
      || !self.inner.keep_running.load(Ordering::Relaxed)
    {
      return Poll::Ready(());
    }

    if let Ok(mut waker) = self.inner.waker.lock() {
      *waker = Some(cx.waker().clone());
    }
    Poll::Pending
  }
}

// ==================== Remote Future Wrapper ====================

pin_project! {
  /// A future wrapper that converts panics into an error output.
  ///
  /// This is a small replacement for `futures::FutureExt::catch_unwind()`.
  struct CatchUnwind<Fut> {
    #[pin]
    future: Fut,
  }
}

impl<Fut> Future for CatchUnwind<Fut>
where
  Fut: Future,
{
  type Output = Result<Fut::Output, Box<dyn Any + Send + 'static>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    let polled = std::panic::catch_unwind(AssertUnwindSafe(|| this.future.poll(cx)));

    match polled {
      Ok(Poll::Pending) => Poll::Pending,
      Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
      Err(e) => Poll::Ready(Err(e)),
    }
  }
}

pin_project! {
    /// A future which sends its output to the corresponding TaskHandle.
    /// Wraps the future with CatchUnwind to convert panics to errors.
    struct Remote<Fut: Future> {
        handle: TaskHandle,
        #[pin]
        future: CatchUnwind<AssertUnwindSafe<Fut>>,
    }
}

impl<Fut: Future<Output = ()>> Future for Remote<Fut> {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    let this = self.project();

    // Check if cancelled
    if !this
      .handle
      .inner
      .keep_running
      .load(Ordering::Relaxed)
    {
      return Poll::Ready(());
    }

    match this.future.poll(cx) {
      Poll::Ready(_result) => {
        // Mark as finished regardless of success or panic
        this.handle.mark_finished();
        Poll::Ready(())
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

/// Create a remote handle for a future
///
/// This wraps the future with CatchUnwind to convert panics to errors
fn remote_handle<Fut: Future<Output = ()>>(future: Fut) -> (Remote<Fut>, TaskHandle) {
  let handle = TaskHandle::new();

  let wrapped =
    Remote { future: CatchUnwind { future: AssertUnwindSafe(future) }, handle: handle.clone() };

  (wrapped, handle)
}

// ==================== Scheduler Trait ====================

/// Core trait for executing schedulable items with optional delays.
///
/// Schedulers are responsible for:
/// - Converting schedulable items into futures via [`Schedulable::into_future`]
/// - Managing execution timing (immediate vs delayed)
/// - Providing cancellation and completion tracking via [`TaskHandle`]
///
/// # Type Parameters
///
/// - `S`: The schedulable item type (must implement `Schedulable<Self>`)
///
/// # Thread Safety Constraints
///
/// Different scheduler implementations enforce different Send requirements:
/// - **`LocalScheduler`**: No Send requirement - can schedule `!Send` types
/// - **`SharedScheduler`**: Requires `Send` - only schedules thread-safe types
///
/// # Implementation Requirements
///
/// Schedulers must be:
/// - `Clone`: Zero-cost to copy (typically zero-sized types)
/// - Stateless: All state should be managed through TaskHandle
///
/// # Example
///
/// ```rust,no_run
/// use rxrust::scheduler::{Duration, Schedulable, Scheduler, TaskHandle};
///
/// #[derive(Clone, Copy, Default)]
/// struct MyScheduler;
///
/// impl<S> Scheduler<S> for MyScheduler
/// where
///   S: 'static + Schedulable<MyScheduler>,
/// {
///   fn schedule(&self, source: S, _delay: Option<Duration>) -> TaskHandle {
///     let _future = source.into_future(self);
///     TaskHandle::finished()
///   }
/// }
/// ```
pub trait Scheduler<S>: Clone {
  /// Schedule a source for execution with optional delay.
  ///
  /// # Parameters
  ///
  /// - `source`: The item to schedule (task, future, etc.)
  /// - `delay`: Optional delay before starting execution
  ///
  /// # Returns
  ///
  /// A [`TaskHandle`] that can be used to cancel or await the scheduled item.
  fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle;
}

// ==================== Default Scheduler Implementations ====================

#[cfg(feature = "scheduler")]
pub mod default_schedulers {
  // Platform-specific imports for tokio or WASM
  #[cfg(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown"))]
  use gloo_timers::future::sleep as platform_sleep;
  #[cfg(not(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown")))]
  use tokio::time::sleep as platform_sleep;
  #[cfg(not(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown")))]
  use tokio::{spawn, task::spawn_local};
  #[cfg(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown"))]
  use wasm_bindgen_futures::spawn_local;

  use super::*;

  /// Single-threaded scheduler for local async execution.
  ///
  /// `LocalScheduler` is designed for scenarios where tasks don't need to be
  /// `Send`:
  /// - UI applications with thread-local state
  /// - Single-threaded async runtimes
  /// - WASM environments (inherently single-threaded)
  ///
  /// # Platform Support
  ///
  /// - **Native**: Uses `tokio::task::spawn_local`
  /// - **WASM**: Uses `wasm_bindgen_futures::spawn_local`
  ///
  /// # Thread Safety
  ///
  /// Tasks scheduled with `LocalScheduler` do **not** require `Send`, allowing
  /// you to schedule `Rc`, `RefCell`, and other `!Send` types safely.
  ///
  /// # Example
  ///
  /// ```rust,no_run
  /// use std::{cell::RefCell, rc::Rc};
  ///
  /// use rxrust::scheduler::{LocalScheduler, Scheduler, Task, TaskState};
  ///
  /// let data = Rc::new(RefCell::new(0)); // !Send type
  /// let scheduler = LocalScheduler;
  ///
  /// let task = Task::new(data, |d| {
  ///   *d.borrow_mut() += 1;
  ///   TaskState::Finished
  /// });
  ///
  /// let handle = scheduler.schedule(task, None);
  /// ```
  #[derive(Clone, Copy, Default)]
  pub struct LocalScheduler;

  impl SleepProvider for LocalScheduler {
    #[cfg(not(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown")))]
    type SleepFuture = tokio::time::Sleep;
    #[cfg(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown"))]
    type SleepFuture = gloo_timers::future::TimeoutFuture;

    fn sleep(&self, duration: Duration) -> Self::SleepFuture { platform_sleep(duration) }
  }

  // Macro to reduce repetitive Scheduler implementations
  macro_rules! impl_scheduler {
    ($sched:ty, $spawn:expr $(, $bound:tt)*) => {
      impl<S> Scheduler<S> for $sched
      where
          S: Schedulable<Self> $(+ $bound)* + 'static,
          S::Future: $($bound +)* 'static,
      {
        fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
            let scheduler = self.clone();
            let future = source.into_future(self);
            let wrapped = async move {
                if let Some(d) = delay {
                    scheduler.sleep(d).await;
                }
                future.await;
            };
            let (remote, handle) = remote_handle(wrapped);
            $spawn(remote);
            handle
        }
      }
    };
  }

  impl_scheduler!(LocalScheduler, spawn_local);

  /// Multi-threaded scheduler for shared async execution.
  ///
  /// `SharedScheduler` is designed for scenarios requiring thread safety:
  /// - Multi-threaded applications
  /// - Tasks that need to run on different threads
  /// - CPU-intensive work that benefits from parallelism
  ///
  /// # Platform Support
  ///
  /// - **Native**: Uses `tokio::spawn` for true multi-threading
  /// - **WASM**: Falls back to `spawn_local` (WASM is single-threaded)
  ///
  /// # Thread Safety
  ///
  /// All tasks scheduled with `SharedScheduler` **must** implement `Send`,
  /// enforced at compile time. This ensures safe execution across threads.
  ///
  /// # Example
  ///
  /// ```rust,no_run
  /// use std::sync::{Arc, Mutex};
  ///
  /// use rxrust::scheduler::{Scheduler, SharedScheduler, Task, TaskState};
  ///
  /// let data = Arc::new(Mutex::new(0)); // Send + Sync type
  /// let scheduler = SharedScheduler;
  ///
  /// let task = Task::new(data, |d| {
  ///   *d.lock().unwrap() += 1;
  ///   TaskState::Finished
  /// });
  ///
  /// let handle = scheduler.schedule(task, None);
  /// ```
  #[derive(Clone, Copy, Default)]
  pub struct SharedScheduler;

  impl SleepProvider for SharedScheduler {
    #[cfg(not(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown")))]
    type SleepFuture = tokio::time::Sleep;
    #[cfg(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown"))]
    type SleepFuture = gloo_timers::future::TimeoutFuture;

    fn sleep(&self, duration: Duration) -> Self::SleepFuture { platform_sleep(duration) }
  }

  #[cfg(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown"))]
  impl_scheduler!(SharedScheduler, spawn_local);

  #[cfg(not(all(target_arch = "wasm32", target_vendor = "unknown", target_os = "unknown")))]
  impl_scheduler!(SharedScheduler, spawn, Send);
}

#[cfg(feature = "scheduler")]
pub use default_schedulers::{LocalScheduler, SharedScheduler};
#[cfg(all(feature = "scheduler", not(target_arch = "wasm32")))]
pub use tokio;

// ==================== Test Scheduler Module ====================

#[cfg(test)]
pub mod test_scheduler;

// ==================== Tests ====================

#[cfg(all(test, feature = "scheduler"))]
mod tests {
  use std::sync::{Arc, Mutex};

  use super::*;

  mod scheduler_tests {
    use super::*;

    #[rxrust_macro::test]
    fn test_task_handle_finished_is_closed() {
      let handle = TaskHandle::finished();
      assert!(handle.is_closed());
    }

    #[rxrust_macro::test]
    fn test_task_creation_and_execution() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let mut task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      task.step();
      assert!(*executed.lock().unwrap());
    }

    #[rxrust_macro::test(local)]
    async fn test_local_scheduler_basic() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let scheduler = LocalScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, None);
      handle.await;

      assert!(*executed.lock().unwrap());
    }

    #[rxrust_macro::test]
    fn test_local_scheduler_with_delay() {
      use std::{cell::Cell, rc::Rc};

      use crate::scheduler::test_scheduler::TestScheduler;

      TestScheduler::init();

      let executed = Rc::new(Cell::new(false));
      let executed_clone = executed.clone();

      let scheduler = TestScheduler;
      let task = Task::new(executed_clone, |flag| {
        flag.set(true);
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, Some(Duration::from_millis(50)));

      assert!(!executed.get());

      TestScheduler::advance_by(Duration::from_millis(30));
      assert!(!executed.get());

      TestScheduler::advance_by(Duration::from_millis(20));
      assert!(executed.get());
      assert!(handle.is_closed());
    }

    #[rxrust_macro::test(local)]
    async fn test_local_scheduler_cancellation() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let scheduler = LocalScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, Some(Duration::from_millis(100)));
      let handle_check = handle.clone();

      // Cancel immediately
      handle.unsubscribe();
      assert!(handle_check.is_closed());

      LocalScheduler
        .sleep(Duration::from_millis(150))
        .await;

      // Task should not have executed
      assert!(!*executed.lock().unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test]
    async fn test_shared_scheduler_basic() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let scheduler = SharedScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, None);
      handle.await;

      assert!(*executed.lock().unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test]
    async fn test_shared_scheduler_with_delay() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();
      let start = Instant::now();

      let scheduler = SharedScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, Some(Duration::from_millis(50)));
      handle.await;

      let elapsed = start.elapsed();
      assert!(*executed.lock().unwrap());
      assert!(elapsed >= Duration::from_millis(50));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test]
    async fn test_shared_scheduler_cancellation() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let scheduler = SharedScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, Some(Duration::from_millis(100)));
      let handle_check = handle.clone();

      // Cancel immediately
      handle.unsubscribe();
      assert!(handle_check.is_closed());

      // Wait a bit to ensure task doesn't execute
      LocalScheduler
        .sleep(Duration::from_millis(150))
        .await;

      // Task should not have executed
      assert!(!*executed.lock().unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test]
    async fn test_shared_scheduler_concurrent_tasks() {
      let counter = Arc::new(Mutex::new(0));
      let scheduler = SharedScheduler;

      let mut handles = vec![];
      for _ in 0..10 {
        let counter_clone = counter.clone();
        let task = Task::new(counter_clone, |cnt| {
          *cnt.lock().unwrap() += 1;
          TaskState::Finished
        });
        let handle = scheduler.schedule(task, None);
        handles.push(handle);
      }

      // Wait for all tasks to complete
      for handle in handles {
        handle.await;
      }

      assert_eq!(*counter.lock().unwrap(), 10);
    }

    #[rxrust_macro::test]
    fn test_scheduler_zero_size_types() {
      use std::mem;

      // Verify that our schedulers are truly zero-sized types
      assert_eq!(mem::size_of::<LocalScheduler>(), 0);
      assert_eq!(mem::size_of::<SharedScheduler>(), 0);

      // Verify they are Copy
      let scheduler1 = LocalScheduler;
      let scheduler2 = scheduler1; // Should work due to Copy trait
      let _ = (scheduler1, scheduler2);

      let scheduler3 = SharedScheduler;
      let scheduler4 = scheduler3;
      let _ = (scheduler3, scheduler4);
    }

    #[rxrust_macro::test(local)]
    async fn test_task_handle_as_future() {
      let executed = Arc::new(Mutex::new(false));
      let executed_clone = executed.clone();

      let scheduler = LocalScheduler;
      let task = Task::new(executed_clone, |flag| {
        *flag.lock().unwrap() = true;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, None);

      let result = handle.await;
      assert_eq!(result, ());

      assert!(*executed.lock().unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test(local)]
    async fn test_panic_handling() {
      let scheduler = LocalScheduler;

      let task = Task::new((), |_| {
        panic!("Intentional panic for testing");
      });

      let handle = scheduler.schedule(task, None);

      let _result = handle.await;

      // Task completes even if it panicked
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[rxrust_macro::test]
    async fn test_shared_scheduler_send_requirement() {
      // This test verifies that SharedScheduler requires Send
      // The fact that this compiles proves the Send bound is working

      let data = Arc::new(Mutex::new(42));
      let data_clone = data.clone();

      let scheduler = SharedScheduler;
      let task = Task::new(data_clone, |d| {
        *d.lock().unwrap() = 100;
        TaskState::Finished
      });

      let handle = scheduler.schedule(task, None);
      handle.await;

      assert_eq!(*data.lock().unwrap(), 100);
    }

    #[rxrust_macro::test]
    fn test_unsubscribe_consumes_handle() {
      // With move semantics, unsubscribe() consumes the handle
      // This is a compile-time guarantee, so this test just documents the behavior
      // Attempting to call unsubscribe twice on the same handle would be a compile
      // error
      let handle = TaskHandle::finished();
      assert!(handle.is_closed());
      handle.unsubscribe();
      // handle is now consumed - calling is_closed() or unsubscribe() again
      // would be a compile error
    }
  }
}
