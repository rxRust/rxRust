use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::sync::mpsc::channel;
use std::thread;

pub struct ThreadScheduler {}

impl Scheduler for ThreadScheduler {
  fn schedule(
    &mut self,
    task: impl FnOnce() -> Box<dyn Subscription + Send + Sync> + Send + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let (sender, receiver) = channel();
    thread::spawn(move || sender.send(task()).unwrap());
    receiver.recv().unwrap()
  }
}
