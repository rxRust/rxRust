//! # Scheduling System
//!
//! This module provides a unified scheduling system for rxRust that works across all platforms
//! (including WebAssembly) using tokio as the foundation. The scheduling system is enabled by
//! the `scheduler` feature.
//! On WebAssembly platforms, `tokio_with_wasm` is used as an adapter to provide tokio-compatible
//! functionality.
//!
//! ## Scheduler Types
//!
//! Two scheduler types are available:
//!
//! - [`LocalScheduler`]: For single-threaded local execution contexts
//! - [`SharedScheduler`]: For multi-threaded shared execution contexts
//!
//! Both schedulers are zero-sized types that the compiler can optimize away completely.
//!
//! ## Usage Examples
//!
//! ### SharedScheduler
//!
//! For multi-threaded execution, use `SharedScheduler` with `#[tokio::main]`:
//!
//! ```rust
//! use rxrust::prelude::*;
//! use rxrust::scheduler::SharedScheduler;
//!
//! #[tokio::main(flavor = "multi_thread")]
//! async fn main() {
//!     let scheduler = SharedScheduler;
//!
//!     observable::from_iter(0..10)
//!         .subscribe_on(scheduler)
//!         .map(|v| v * 2)
//!         .observe_on_threads(scheduler)
//!         .subscribe(|v| println!("{}", v));
//! }
//! ```
//!
//! ### LocalScheduler
//!
//! For single-threaded local execution, you must first enter a `LocalSet` context:
//!
//! ```rust
//! use rxrust::prelude::*;
//! use rxrust::scheduler::LocalScheduler;
//! use LocalSet;
//!
//! #[tokio::main]
//! async fn main() {
//!     let local_set = LocalSet::new();
//!     let _guard = local_set.enter();
//!
//!     // Now you can use LocalScheduler
//!     observable::from_iter(0..10)
//!         .observe_on(LocalScheduler)
//!         .subscribe(|v| println!("{}", v));
//!
//!     // Wait for all local tasks to complete
//!     local_set.await;
//! }
//! ```
//!
//! #### Using LocalRuntime (unstable feature)
//!
//! For more convenient usage with LocalScheduler, you can use Tokio's unstable `LocalRuntime`:
//!
//! ```rust ignore
//! #![cfg(tokio_unstable)]
//!
//! use rxrust::prelude::*;
//! use rxrust::scheduler::LocalScheduler;
//!
//! #[tokio::main(flavor = "local")]
//! async fn main() {
//!   // No need for LocalSet when using LocalRuntime
//!   observable::from_iter(0..10)
//!       .observe_on(LocalScheduler)
//!       .subscribe(|v| println!("{}", v));
//!   tokio::time::sleep(Duration::from_millis(100)).await;
//! }
//! ```
//!
//! ## Thread Safety Considerations
//!
//! When using `SharedScheduler` with operations that require `Send` (like `delay_threads`,
//! `observe_on_threads`), ensure your observers and data are thread-safe. Use `MutArc`
//! instead of `MutRc` for shared state in multi-threaded contexts.
//!
//! For `LocalScheduler`, you can use `MutRc` since execution stays on a single thread.
//!
//! ## Custom Scheduler Implementation
//!
//! You can implement your own scheduler by disabling the `scheduler` feature and implementing
//! the [`Scheduler`] trait:

use crate::{
  prelude::*,
  rc::{MutArc, RcDeref, RcDerefMut},
};
use futures::{future::CatchUnwind, ready, FutureExt};
use pin_project_lite::pin_project;
use std::{
  any::Any,
  fmt,
  future::Future,
  mem::swap,
  panic::{self, AssertUnwindSafe},
  pin::Pin,
  task::{Context, Poll},
  time::Instant,
};

#[cfg(all(
  target_arch = "wasm32",
  target_vendor = "unknown",
  target_os = "unknown"
))]
pub type BoxFuture<'a, T> =
  std::pin::Pin<Box<dyn futures::Future<Output = T> + 'a>>;

#[cfg(not(all(
  target_arch = "wasm32",
  target_vendor = "unknown",
  target_os = "unknown"
)))]
pub type BoxFuture<'a, T> = futures::future::BoxFuture<'a, T>;

pub struct TaskHandle<T>(MutArc<HandleInfo<T>>);
struct HandleInfo<T> {
  keep_running: bool,
  value: Option<Result<T, Box<dyn Any + Send>>>,
}

/// Trait for implementing custom schedulers.
///
/// This trait allows you to implement your own scheduling logic for executing
/// asynchronous tasks. When the `scheduler` feature is disabled, you can implement
/// this trait to provide custom scheduling behavior.
///
/// # Type Parameters
///
/// - `T`: The future type to be scheduled
///
/// # Example
///
/// ```rust
/// use rxrust::scheduler::{Scheduler, TaskHandle};
/// use std::time::Duration;
/// use std::future::Future;
///
/// struct MyCustomScheduler;
///
/// impl<T> Scheduler<T> for MyCustomScheduler
/// where
///     T: Future + Send + 'static,
///     T::Output: Send,
/// {
///     fn schedule(&self, task: T, delay: Option<Duration>) -> TaskHandle<T::Output> {
///         // Your custom scheduling logic here
///         // This could integrate with other async runtimes, thread pools, etc.
///         todo!("Implement your scheduling logic")
///     }
/// }
/// ```
pub trait Scheduler<T>
where
  T: Future,
{
  /// Schedule a task to be executed, optionally with a delay.
  ///
  /// # Parameters
  ///
  /// - `task`: The future to be executed
  /// - `delay`: Optional duration to wait before executing the task
  ///
  /// # Returns
  ///
  /// A [`TaskHandle`] that can be used to cancel or monitor the scheduled task
  fn schedule(&self, task: T, delay: Option<Duration>)
    -> TaskHandle<T::Output>;
}

#[cfg(feature = "scheduler")]
mod scheduler_impl {
  use super::*;
  #[cfg(all(
    target_arch = "wasm32",
    target_vendor = "unknown",
    target_os = "unknown"
  ))]
  pub use gloo_timers::future::sleep;
  #[cfg(all(
    target_arch = "wasm32",
    target_vendor = "unknown",
    target_os = "unknown"
  ))]
  pub use wasm_bindgen_futures::{spawn_local as spawn, spawn_local};

  #[cfg(not(all(
    target_arch = "wasm32",
    target_vendor = "unknown",
    target_os = "unknown"
  )))]
  pub use tokio::{
    spawn,
    task::{spawn_local, LocalSet},
    time::{self, sleep},
  };

  /// A scheduler for local execution contexts.
  ///
  /// This scheduler uses `spawn_local` for single-threaded execution
  /// across all platforms. On WebAssembly platforms, `tokio_with_wasm` provides the
  /// compatibility layer that enables tokio functionality.
  ///
  /// **Important**: Before using `LocalScheduler`, you must enter a `LocalSet` context
  /// using `LocalSet::enter()` to create a guard, or use the unstable `LocalRuntime`.
  ///
  /// This is a zero-sized type that the compiler can optimize away completely.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  /// use rxrust::scheduler::{LocalScheduler, LocalSet};
  ///
  /// #[tokio::main]
  /// async fn main() {
  ///     let local_set = LocalSet::new();
  ///     let _guard = local_set.enter();
  ///
  ///     // Now you can use LocalScheduler
  ///     observable::from_iter(0..10)
  ///         .observe_on(LocalScheduler)
  ///         .subscribe(|v| println!("{}", v));
  ///
  ///     local_set.await;
  /// }
  /// ```
  #[derive(Debug, Clone, Copy)]
  pub struct LocalScheduler;

  /// A scheduler for shared/multi-threaded execution contexts.
  ///
  /// This scheduler uses `tokio::spawn` for multi-threaded execution across all platforms.
  /// On WebAssembly platforms, `tokio_with_wasm` provides the compatibility layer that
  /// enables tokio functionality. Since true multi-threading is not available in WebAssembly,
  /// this behaves identically to `LocalScheduler` on WASM platforms.
  ///
  /// This is a zero-sized type that the compiler can optimize away completely.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  /// use rxrust::scheduler::SharedScheduler;
  ///
  /// #[tokio::main(flavor = "multi_thread")]
  /// async fn main() {
  ///     let scheduler = SharedScheduler;
  ///
  ///     observable::from_iter(0..10)
  ///         .subscribe_on(scheduler)
  ///         .observe_on_threads(scheduler)
  ///         .subscribe(|v| println!("{}", v));
  /// }
  /// ```
  #[derive(Debug, Clone, Copy)]
  pub struct SharedScheduler;

  // Unified scheduler implementation using tokio interface
  #[cfg(feature = "scheduler")]
  impl<T> Scheduler<T> for LocalScheduler
  where
    T: Future + 'static,
    T::Output: TaskReturn,
  {
    fn schedule(
      &self,
      task: T,
      delay: Option<Duration>,
    ) -> TaskHandle<T::Output> {
      let (fut, handle) = schedule_task_with_delay(task, delay);
      spawn_local(fut);
      handle
    }
  }

  #[cfg(feature = "scheduler")]
  impl<T> Scheduler<T> for SharedScheduler
  where
    T: Future + Send + 'static,
    T::Output: TaskReturn + Send,
  {
    fn schedule(
      &self,
      task: T,
      delay: Option<Duration>,
    ) -> TaskHandle<T::Output> {
      let (fut, handle) = schedule_task_with_delay(task, delay);
      spawn(fut);
      handle
    }
  }
}

#[cfg(feature = "scheduler")]
pub use scheduler_impl::*;

pin_project! {
  pub struct OnceTask<Args, R> {
    func: fn(Args) -> R,
    args: Option<Args>,
  }
}

pin_project! {
  pub struct FutureTask<F: Future, Args, R> {
    #[pin]
    future: F,
    task: fn(F::Output, Args)->R,
    args: Option<Args>,
  }
}

pin_project! {
  pub struct RepeatTask<Args> {
    fur: BoxFuture<'static, ()>,
    interval: Duration,
    // the task to do and return if you want the task continue repeat.
    task: fn(&mut Args, usize)-> bool,
    args: Args,
    seq: usize,
    next_execution_time: Option<Instant>,
  }
}

impl<Args, R> OnceTask<Args, R> {
  #[inline]
  pub fn new(func: fn(Args) -> R, args: Args) -> Self {
    OnceTask { func, args: Some(args) }
  }
}

impl<F: Future, Args, R> FutureTask<F, Args, R> {
  pub fn new(future: F, task: fn(F::Output, Args) -> R, args: Args) -> Self {
    Self { future, task, args: Some(args) }
  }
}

impl<Args, R: TaskReturn> Future for OnceTask<Args, R> {
  type Output = R;

  fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    let args = this.args.take().unwrap();
    Poll::Ready((*this.func)(args))
  }
}

impl<Args> Future for RepeatTask<Args> {
  type Output = NormalReturn<()>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    loop {
      ready!(self.fur.poll_unpin(cx));

      let next = match self.next_execution_time {
        Some(time) => time + self.interval,
        None => Instant::now() + self.interval,
      };

      // Execute the user's task
      {
        let this = self.as_mut().project();
        if (*this.task)(this.args, *this.seq) {
          *this.seq += 1;
        } else {
          return Poll::Ready(NormalReturn::new(()));
        }
      }

      // Calculate the next execution time
      self.next_execution_time = Some(next);

      // Create new sleep future with the calculated duration
      let mut fur: BoxFuture<'static, ()> =
        Box::pin(sleep(next - Instant::now()));
      swap(&mut self.fur, &mut fur);
    }
  }
}

impl<F, Args, R: TaskReturn> Future for FutureTask<F, Args, R>
where
  F: Future,
{
  type Output = R;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match this.future.poll(cx) {
      Poll::Ready(v) => {
        let args = this.args.take().unwrap();
        Poll::Ready((*this.task)(v, args))
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

impl<Args> RepeatTask<Args> {
  pub fn new(
    dur: Duration,
    task: fn(&mut Args, usize) -> bool,
    args: Args,
  ) -> Self {
    let now = Instant::now();
    Self {
      fur: Box::pin(sleep(dur).map(|_| ())) as BoxFuture<'static, ()>,
      interval: dur,
      task,
      args,
      seq: 0,
      next_execution_time: Some(now + dur),
    }
  }
}

pub struct SubscribeReturn<T: Subscription>(T);
pub struct NormalReturn<T>(T);

impl<T> NormalReturn<T> {
  #[inline]
  pub fn new(v: T) -> Self {
    Self(v)
  }
}

impl<T: Subscription> SubscribeReturn<T> {
  #[inline]
  pub fn new(v: T) -> Self {
    Self(v)
  }
}

impl<T> TaskHandle<T> {
  pub fn value_handle(v: T) -> Self {
    Self(MutArc::own(HandleInfo {
      keep_running: true,
      value: Some(Ok(v)),
    }))
  }
}
trait TaskReturn {}

impl<T: Subscription> TaskReturn for SubscribeReturn<T> {}
impl<T> TaskReturn for NormalReturn<T> {}

impl<T: 'static> Subscription for TaskHandle<NormalReturn<T>> {
  #[inline]
  fn unsubscribe(self) {
    let mut inner = self.0.rc_deref_mut();
    inner.keep_running = false;
    inner.value.take();
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.0.rc_deref().value.is_some()
  }
}

impl<T: Subscription + 'static> Subscription
  for TaskHandle<SubscribeReturn<T>>
{
  fn unsubscribe(self) {
    let mut info = self.0.rc_deref_mut();
    info.keep_running = false;
    match info.value.take() {
      Some(Ok(v)) => v.0.unsubscribe(),
      Some(Err(e)) => panic::resume_unwind(e),
      None => {}
    }
  }

  #[inline]
  fn is_closed(&self) -> bool {
    let info = self.0.rc_deref();
    match info.value.as_ref() {
      Some(Ok(u)) => u.0.is_closed(),
      _ => false,
    }
  }
}

pin_project! {
  /// A future which sends its output to the corresponding `RemoteHandle`.
  /// Created by [`remote_handle`](crate::future::FutureExt::remote_handle).
  #[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
  struct Remote<Fut: Future> {
      handle_info: MutArc<HandleInfo<Fut::Output>>,
      #[pin]
      future: CatchUnwind<AssertUnwindSafe<Fut>>,
  }
}

impl<Fut: Future + fmt::Debug> fmt::Debug for Remote<Fut> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("Remote").field(&self.future).finish()
  }
}

impl<Fut: Future> Future for Remote<Fut> {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    let this = self.project();

    let mut info = this.handle_info.rc_deref_mut();
    if !info.keep_running {
      // Cancelled, bail out
      return Poll::Ready(());
    }
    info.value = Some(ready!(this.future.poll(cx)));

    Poll::Ready(())
  }
}

fn remote_handle<Fut: Future>(
  future: Fut,
) -> (Remote<Fut>, TaskHandle<Fut::Output>) {
  let handle =
    TaskHandle(MutArc::own(HandleInfo { keep_running: true, value: None }));

  // Unwind Safety: See the docs for RemoteHandle.
  let wrapped = Remote {
    future: AssertUnwindSafe(future).catch_unwind(),
    handle_info: handle.0.clone(),
  };

  (wrapped, handle)
}

/// Helper function to create a delayed task with remote handle
fn schedule_task_with_delay<T>(
  task: T,
  delay: Option<Duration>,
) -> (
  Remote<impl Future<Output = T::Output>>,
  TaskHandle<T::Output>,
)
where
  T: Future,
{
  let fut = async move {
    if let Some(dur) = delay {
      sleep(dur).await;
    }
    task.await
  };
  remote_handle(fut)
}

/// Test utilities for the scheduler module
#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use std::sync::{Arc, Mutex};

  #[cfg(feature = "scheduler")]
  mod unified_schedulers {
    use super::*;

    #[tokio::test]
    async fn test_local_scheduler_basic_functionality() {
      use std::sync::{Arc, Mutex};

      let results = Arc::new(Mutex::new(Vec::new()));
      let results_clone = results.clone();

      let local_set = LocalSet::new();
      let _guard = local_set.enter();
      observable::from_iter(0..5)
        .observe_on(LocalScheduler)
        .subscribe(move |item| {
          results_clone.lock().unwrap().push(item);
        });

      local_set.await;
      assert_eq!(*results.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_shared_scheduler_basic_functionality() {
      use std::sync::{Arc, Mutex};

      let scheduler = crate::scheduler::SharedScheduler;
      let results = Arc::new(Mutex::new(Vec::new()));
      let results_clone = results.clone();

      let (o, status) = observable::from_iter(0..5)
        .observe_on_threads(scheduler)
        .complete_status();

      o.subscribe(move |item| {
        results_clone.lock().unwrap().push(item);
      });

      status.wait_completed().await;
      assert_eq!(*results.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_scheduler_without_observe_on() {
      use std::sync::{Arc, Mutex};

      // Test that our schedulers work even without observe_on
      let results = Arc::new(Mutex::new(Vec::new()));
      let results_clone = results.clone();

      let (o, status) = observable::from_iter(1..=3).complete_status();

      o.subscribe(move |item| {
        results_clone.lock().unwrap().push(item);
      });

      status.wait_completed().await;
      assert_eq!(*results.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_scheduler_zero_size_types() {
      use std::mem;

      // Verify that our schedulers are truly zero-sized types
      assert_eq!(mem::size_of::<crate::scheduler::LocalScheduler>(), 0);
      assert_eq!(mem::size_of::<crate::scheduler::SharedScheduler>(), 0);

      // Verify they are Copy
      let scheduler1 = crate::scheduler::LocalScheduler;
      let scheduler2 = scheduler1; // Should work due to Copy trait
      let _ = (scheduler1, scheduler2);

      let scheduler3 = crate::scheduler::SharedScheduler;
      let scheduler4 = scheduler3;
      let _ = (scheduler3, scheduler4);
    }

    #[tokio::test]
    async fn test_scheduler_clone_behavior() {
      // Test that schedulers can be cloned/copied
      let scheduler = crate::scheduler::SharedScheduler;
      let scheduler_clone = scheduler;

      let results = Arc::new(Mutex::new(Vec::new()));
      let results_clone = results.clone();

      let (o, status) = observable::from_iter(10..15)
        .observe_on_threads(scheduler_clone)
        .complete_status();

      o.subscribe(move |item| {
        results_clone.lock().unwrap().push(item);
      });

      status.wait_completed().await;

      assert_eq!(*results.lock().unwrap(), vec![10, 11, 12, 13, 14]);
    }

    #[tokio::test]
    async fn test_scheduler_demonstrates_user_control() {
      // Test to show that user is responsible for runtime management
      // With tokio_with_wasm, both WASM and non-WASM use the same tokio interface
      let scheduler = crate::scheduler::LocalScheduler;

      let results = Arc::new(Mutex::new(Vec::new()));
      let results_clone = results.clone();

      let local_set = LocalSet::new();
      let _guard = local_set.enter();

      observable::from_iter(100..105)
        .observe_on(scheduler)
        .subscribe(move |item| {
          results_clone.lock().unwrap().push(item);
        });

      local_set.await;
      assert_eq!(*results.lock().unwrap(), vec![100, 101, 102, 103, 104]);
    }

    #[tokio::test]
    async fn test_repeat_task_fixed_interval_with_work() {
      use std::time::{Duration, Instant};

      let start_time = Instant::now();
      let local_set = LocalSet::new();
      let _guard = local_set.enter();

      observable::interval(Duration::from_millis(10), LocalScheduler)
        .take(5)
        .subscribe(move |_| {
          let work_start = Instant::now();
          while work_start.elapsed() < Duration::from_millis(12) {
            // Simulate work by busy-waiting
          }
        });

      local_set.await;

      let elapsed = start_time.elapsed();
      println!("Total elapsed time: {:?}", elapsed);
      assert!(
        // before fix use:  (10 + 12) * 5 = 110ms
        // after fix: 10 + 12 * 5 = 70ms, give some buffer
        elapsed < Duration::from_millis(85),
        "Expected < 75ms, got {elapsed:?}",
      );
    }
  }
}
