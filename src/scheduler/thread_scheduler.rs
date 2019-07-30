use crate::scheduler::Scheduler;
use std::sync::mpsc::channel;
use std::thread;

pub struct ThreadScheduler {}

impl Scheduler for ThreadScheduler {
  fn schedule<T: Send + Sync + 'static, R: Send + Sync + 'static>(
    &self,
    task: impl FnOnce(Option<T>) -> R + Send + 'static,
    state: Option<T>,
  ) -> R {
    let (sender, receiver) = channel();
    thread::spawn(move || sender.send(task(state)).unwrap());
    receiver.recv().unwrap()
  }
}
