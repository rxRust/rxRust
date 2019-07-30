mod thread_scheduler;
pub use thread_scheduler::ThreadScheduler;

/// A Scheduler is an object to order task and schedule their execution.
pub trait Scheduler {
  fn schedule<T: Send + Sync + 'static, R: Send + Sync + 'static>(
    &self,
    task: impl FnOnce(Option<T>) -> R + Send + 'static,
    state: Option<T>,
  ) -> R;
}

/// Returns a Scheduler instance that creates a new thread for each unit of
/// work.
pub fn new_thread() -> ThreadScheduler { ThreadScheduler {} }
