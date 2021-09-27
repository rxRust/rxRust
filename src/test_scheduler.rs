#![cfg(test)]
use crate::prelude::{LocalScheduler, SpawnHandle, SubscriptionLike};
use futures::future::AbortHandle;
use std::future::Future;
use std::ops::{Add, Sub};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ManualScheduler {
  clock: Arc<RwLock<FakeClock>>,
  repeating_task: Arc<RwLock<Vec<Arc<RwLock<RepeatingTask>>>>>,
  oneshot_tasks: Arc<RwLock<Vec<OneshotTask>>>,
}

struct FakeClock {
  current_time: Instant,
}

impl FakeClock {
  fn instant(&self) -> Instant { self.current_time }

  pub fn new(time: Instant) -> FakeClock { FakeClock { current_time: time } }

  pub fn advance(&mut self, duration: Duration) {
    self.current_time = self.current_time.add(duration);
  }
}

struct RepeatingTask {
  task: Box<dyn FnMut(usize)>,
  delay: Duration,
  last_time: Instant,
  invokes: Arc<RwLock<usize>>,
  cancel: SpawnHandle,
}

impl RepeatingTask {
  pub fn invoke(&mut self) {
    let invokes = *self.invokes.read().unwrap();
    (self.task)(invokes);
    *self.invokes.write().unwrap() += 1;
  }
}

struct OneshotTask {
  task: Box<dyn FnOnce()>,
  delay: Duration,
  start: Instant,
  cancel: SpawnHandle,
}

impl LocalScheduler for ManualScheduler {
  fn spawn<Fut>(&self, future: Fut)
  where
    Fut: Future<Output = ()> + 'static,
  {
    futures::executor::block_on(future);
  }

  fn schedule<S: 'static>(
    &self,
    task: impl FnOnce(S) + 'static,
    delay: Option<Duration>,
    state: S,
  ) -> SpawnHandle {
    let handle = SpawnHandle::new(AbortHandle::new_pair().0);
    (*self.oneshot_tasks.write().unwrap()).push(OneshotTask {
      task: Box::new(|| {
        task(state);
      }),
      delay: delay.unwrap_or_else(|| Duration::from_micros(0)),
      start: (*self.clock.read().unwrap()).instant(),
      cancel: handle.clone(),
    });
    handle
  }

  fn schedule_repeating(
    &self,
    task: impl FnMut(usize) + 'static,
    delay: Duration,
    at: Option<Instant>,
  ) -> SpawnHandle {
    let handle = SpawnHandle::new(AbortHandle::new_pair().0);
    (*self.repeating_task.write().unwrap()).push(Arc::new(RwLock::new(
      RepeatingTask {
        task: Box::new(task),
        invokes: Arc::new(RwLock::new(0)),
        delay,
        last_time: at
          .map(|t| t.sub(delay))
          .unwrap_or_else(|| (*self.clock.read().unwrap()).instant()),
        cancel: handle.clone(),
      },
    )));
    handle
  }
}

impl ManualScheduler {
  pub fn new(now: Instant) -> ManualScheduler {
    ManualScheduler {
      clock: Arc::new(RwLock::new(FakeClock::new(now))),
      repeating_task: Arc::new(RwLock::new(vec![])),
      oneshot_tasks: Arc::new(RwLock::new(vec![])),
    }
  }

  pub fn now() -> ManualScheduler { ManualScheduler::new(Instant::now()) }

  pub fn advance(&self, time: Duration) {
    self.clock.write().unwrap().advance(time);
  }

  pub fn advance_and_run(&self, advance_by: Duration, times: usize) {
    for _ in 0..times {
      self.advance(advance_by);
      self.run_tasks();
    }
  }

  pub fn run_tasks(&self) {
    // Minor race condition below that would result in incomplete processing if
    // multiple threads cancel or add tasks while in loop, resolves on next run
    let oneshots = (*self.oneshot_tasks.read().unwrap()).len();
    for _ in 0..oneshots {
      let t = (*self.oneshot_tasks.write().unwrap()).pop().unwrap();
      if !t.cancel.is_closed() {
        let next_time: Instant = t.start.add(t.delay);
        let clock_time = (*self.clock.read().unwrap()).instant();
        if next_time < clock_time {
          (t.task)();
        } else {
          (*self.oneshot_tasks.write().unwrap()).push(t);
        }
      }
    }

    // Minor race condition below that would result in incomplete processing if
    // multiple threads cancel or add tasks while in loop, resolves on next run
    let repeated = (*self.repeating_task.read().unwrap()).len();
    for _ in 0..repeated {
      let t = (*self.repeating_task.write().unwrap()).pop().unwrap();
      if !t.read().unwrap().cancel.is_closed() {
        self.repeating_task.write().unwrap().push(t);
      }
    }

    for task in &(*self.repeating_task.read().unwrap()) {
      let task_c = Arc::clone(task);
      let delay = (*task_c.read().unwrap()).delay;

      let mut prev = task_c.read().unwrap().last_time;
      let next_time: Instant = prev.add(delay);
      let clock_time = (*self.clock.read().unwrap()).instant();
      if next_time <= clock_time {
        let delay_millis = delay.as_millis();
        let passed_millis = clock_time.sub(prev).as_millis();
        let wanted_invokes = passed_millis / delay_millis;
        for _ in 0..wanted_invokes {
          task_c.write().unwrap().invoke();
          prev = prev.add(delay);
        }
        if wanted_invokes > 0 {
          (*task_c.write().unwrap()).last_time = prev;
        }
      }
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_scheduler::ManualScheduler;

  #[test]
  fn spawns_sync() {
    let scheduler = ManualScheduler::now();
    let invokes = Arc::new(Mutex::new(0));
    let invokes_c = invokes.clone();
    let fut = futures::future::lazy(move |_| *invokes_c.lock().unwrap() += 1);
    scheduler.spawn(fut);
    assert_eq!(1, *invokes.lock().unwrap());
  }

  #[test]
  fn schedule_repeating() {
    let time = Instant::now();
    let scheduler = ManualScheduler::new(time);
    let invokes = Arc::new(Mutex::new(0));
    let invokes_c = invokes.clone();
    let delay = Duration::from_millis(100);
    let mut handle = scheduler.schedule_repeating(
      move |_| *invokes_c.clone().lock().unwrap() += 1,
      delay,
      Some(time.add(Duration::from_millis(5))),
    );
    scheduler.run_tasks();
    assert_eq!(0, *invokes.lock().unwrap());
    scheduler.advance(Duration::from_millis(5));
    scheduler.run_tasks();
    assert_eq!(1, *invokes.lock().unwrap());
    scheduler.advance(delay);
    scheduler.run_tasks();
    assert_eq!(2, *invokes.lock().unwrap());
    scheduler.run_tasks();
    scheduler.advance(delay);
    scheduler.run_tasks();
    assert_eq!(3, *invokes.lock().unwrap());
    scheduler.advance(10 * delay);
    scheduler.run_tasks();
    assert_eq!(13, *invokes.lock().unwrap());
    handle.unsubscribe();
    assert!(handle.is_closed());
    scheduler.advance(10 * delay);
    scheduler.run_tasks();
    assert_eq!(13, *invokes.lock().unwrap());
  }

  #[test]
  fn schedule() {
    let scheduler = ManualScheduler::now();
    let invokes = Arc::new(Mutex::new(0));
    let invokes_c = invokes.clone();
    let delay = Duration::from_millis(100);
    scheduler.schedule(
      move |_| *invokes_c.lock().unwrap() += 1,
      Some(delay),
      1,
    );
    scheduler.advance(delay);
    scheduler.run_tasks();
    assert_eq!(0, *invokes.lock().unwrap());
    scheduler.advance(Duration::from_millis(1));
    scheduler.run_tasks();
    scheduler.advance(delay);
    scheduler.run_tasks();
    assert_eq!(1, *invokes.lock().unwrap());
    scheduler.advance(10 * delay);
    scheduler.run_tasks();
    assert_eq!(1, *invokes.lock().unwrap());
  }

  #[test]
  fn schedule_no_schedule_after_unsub() {
    let scheduler = ManualScheduler::now();
    let invokes = Arc::new(Mutex::new(0));
    let invokes_c = invokes.clone();
    let delay = Duration::from_millis(100);
    let mut handle = scheduler.schedule(
      move |_| *invokes_c.lock().unwrap() += 1,
      Some(delay),
      1,
    );
    scheduler.advance(delay);
    scheduler.run_tasks();
    assert_eq!(0, *invokes.lock().unwrap());
    handle.unsubscribe();
    assert!(handle.is_closed());
    scheduler.advance(Duration::from_millis(1));
    scheduler.run_tasks();
    assert_eq!(0, *invokes.lock().unwrap());
  }
}
