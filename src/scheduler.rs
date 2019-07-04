use crate::prelude::*;
mod thread_scheduler;
pub use thread_scheduler::ThreadScheduler;

/// A Scheduler is an object to order task and schedule their execution.
pub trait Scheduler {
  fn schedule(
    &mut self,
    task: impl FnOnce() -> Box<dyn Subscription + Send + Sync> + Send + 'static,
  ) -> Box<dyn Subscription + Send + Sync>;
}

/// Returns a Scheduler instance that creates a new thread for each unit of
/// work.
pub fn new_thread() -> ThreadScheduler { ThreadScheduler {} }
