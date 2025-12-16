//! Example: Custom Scheduler Injection
//!
//! This example demonstrates the "Zero-Cost Scheduler Injection" pattern.
//! We define a custom scheduler that logs events and executes tasks immediately
//! (blocking the thread for delays), and then inject it into rxRust.

use std::time::Duration;

use rxrust::{
  prelude::*,
  scheduler::{Schedulable, Scheduler, SleepProvider, TaskHandle},
};

// ==================================================================================
// 1. Define the Custom Scheduler
// ==================================================================================

#[derive(Clone, Default)]
pub struct VerboseScheduler;

// 2. Implement SleepProvider
// This allows rxRust's internal `Task<S>` to automatically become `Schedulable`
// on our scheduler. The `Task` state machine will call this to handle delays.
impl SleepProvider for VerboseScheduler {
  type SleepFuture = std::future::Ready<()>;

  fn sleep(&self, duration: Duration) -> Self::SleepFuture {
    println!("[VerboseScheduler] Task requested sleep for {:?} (Blocking thread...)", duration);
    std::thread::sleep(duration);
    std::future::ready(())
  }
}

// 3. Implement Scheduler for ANY Schedulable item
// This covers:
// - Tasks (which become schedulable via SleepProvider)
// - Futures (which are natively schedulable)
impl<S> Scheduler<S> for VerboseScheduler
where
  S: Schedulable<VerboseScheduler> + 'static,
{
  fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
    println!("[VerboseScheduler] Scheduling new item. Initial Delay: {:?}", delay);

    // Handle initial delay
    if let Some(d) = delay {
      println!("[VerboseScheduler] Waiting initial delay (Blocking thread...)");
      std::thread::sleep(d);
    }

    // Convert the source into a Future
    // This works for both Task<S> (via SleepProvider logic) and raw Futures.
    let future = source.into_future(self);

    // Run the future immediately (Synchronous execution for demo)
    let waker = futures::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);

    // Pin the future so we can poll it
    let mut boxed = Box::pin(future);

    println!("[VerboseScheduler] Polling future...");
    match boxed.as_mut().poll(&mut cx) {
      std::task::Poll::Ready(_) => println!("[VerboseScheduler] Future completed."),
      std::task::Poll::Pending => println!(
        "[VerboseScheduler] Future yielded (Note: Simplistic demo scheduler does not re-poll)."
      ),
    }

    TaskHandle::finished()
  }
}

// ==================================================================================
// 4. The Type Alias (The "Secret Sauce")
// ==================================================================================

// This creates a new "Context" type that behaves exactly like `Local`
// but uses `VerboseScheduler` for all time-based operations.
type VerboseRx<T> = rxrust::context::LocalCtx<T, VerboseScheduler>;

// ==================================================================================
// 5. Usage
// ==================================================================================

fn main() {
  println!("--- Starting Custom Scheduler Example ---");

  // We use `VerboseRx` instead of `Local`.
  VerboseRx::of(10)
    .map(|v| v * 2)
    // This delay uses VerboseScheduler::sleep
    .delay(Duration::from_millis(500))
    .subscribe(|v| {
      println!("Consumer received value: {}", v);
    });

  // Wait a bit just to be sure (though our scheduler is blocking, so strictly not
  // needed)
  std::thread::sleep(Duration::from_millis(100));

  println!("--- Example Finished ---");
}
