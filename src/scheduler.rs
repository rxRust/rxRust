use crate::prelude::*;
mod thread_scheduler;
use thread_scheduler::new_thread_schedule;
mod thread_pool_scheduler;
use crate::observable::{from_future::DEFAULT_RUNTIME, interval::SpawnHandle};
use futures::prelude::*;
use futures::task::SpawnExt;
use futures_timer::Delay;
use std::time::Duration;
use thread_pool_scheduler::thread_pool_schedule;

/// A Scheduler is an object to order task and schedule their execution.
pub trait Scheduler {
  fn schedule<T: Send + 'static>(
    &self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SharedSubscription;
}

pub enum Schedulers {
  /// NewThread Scheduler always creates a new thread for each unit of work.
  NewThread,
  /// ThreadPool dispatch task to the thread pool to execute task.
  ThreadPool,
}

impl Scheduler for Schedulers {
  fn schedule<T: Send + 'static>(
    &self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SharedSubscription {
    match self {
      Schedulers::NewThread => new_thread_schedule(task, delay, state),
      Schedulers::ThreadPool => thread_pool_schedule(task, delay, state),
    }
  }
}

pub fn delay_task(
  delay: Duration,
  task: impl FnOnce() + Send + 'static,
) -> SpawnHandle<Result<(), std::io::Error>> {
  let f = Delay::new(delay).inspect(|_| {
    task();
  });
  let handle = DEFAULT_RUNTIME
    .lock()
    .unwrap()
    .spawn_with_handle(f)
    .expect("spawn task to thread pool failed.");
  SpawnHandle::new(handle)
}

#[cfg(test)]
mod test {
  extern crate test;
  use crate::ops::ObserveOn;
  use crate::prelude::*;
  use crate::scheduler::Schedulers;
  use std::f32;
  use std::sync::{Arc, Mutex};
  use test::Bencher;

  #[bench]
  fn pool(b: &mut Bencher) { b.iter(|| sum_of_sqrt(Schedulers::ThreadPool)) }

  #[bench]
  fn new_thread(b: &mut Bencher) {
    b.iter(|| sum_of_sqrt(Schedulers::NewThread))
  }

  // #[bench]
  // fn sync(b: &mut Bencher) { b.iter(|| sum_of_sqrt(Schedulers::Sync)) }

  fn sum_of_sqrt(scheduler: Schedulers) {
    let sum = Arc::new(Mutex::new(0.));
    observable::from_iter(0..10)
      .observe_on(scheduler)
      .subscribe(move |v| {
        *sum.lock().unwrap() =
          (0..1000).fold((v as f32).sqrt(), |acc, _| acc.sqrt());
      });
  }
}
