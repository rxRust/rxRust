use std::thread;

pub(crate) fn new_thread_schedule<T: Send + Sync + 'static>(
  task: impl FnOnce(Option<T>) + Send + 'static,
  state: Option<T>,
) {
  thread::spawn(move || task(state));
}
