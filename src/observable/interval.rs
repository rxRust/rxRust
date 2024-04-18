use crate::prelude::*;
use std::convert::Infallible;

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
pub fn interval<S>(dur: Duration, scheduler: S) -> IntervalObservable<S> {
  IntervalObservable { dur, delay: None, scheduler }
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
pub fn interval_at<S>(
  at: Instant,
  dur: Duration,
  scheduler: S,
) -> IntervalObservable<S> {
  let now = Instant::now();
  let delay = if at > now {
    at - now
  } else {
    Duration::from_micros(0)
  };
  IntervalObservable { scheduler, dur, delay: Some(delay) }
}

#[derive(Clone)]
pub struct IntervalObservable<S> {
  scheduler: S,
  dur: Duration,
  delay: Option<Duration>,
}

impl<S, O> Observable<usize, Infallible, O> for IntervalObservable<S>
where
  O: Observer<usize, Infallible>,
  S: Scheduler<RepeatTask<O>>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { scheduler, dur, delay } = self;
    scheduler.schedule(RepeatTask::new(dur, interval_task, observer), delay)
  }
}

impl<S> ObservableExt<usize, Infallible> for IntervalObservable<S> {}

fn interval_task<O>(observer: &mut O, seq: usize) -> bool
where
  O: Observer<usize, Infallible>,
{
  if !observer.is_finished() {
    observer.next(seq);
    true
  } else {
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::LocalPool;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  use std::sync::{Arc, Mutex};

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared() {
    use crate::ops::complete_status::CompleteStatus;

    let millis = Arc::new(Mutex::new(0));
    let c_millis = millis.clone();
    let stamp = Instant::now();
    let pool = ThreadPool::new().unwrap();

    let (o, status) = interval(Duration::from_millis(1), pool)
      .take(5) // Will block forever if we don't limit emissions
      .complete_status();
    o.subscribe(move |_| {
      *millis.lock().unwrap() += 1;
    });
    CompleteStatus::wait_for_end(status);

    assert_eq!(*c_millis.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let stamp = Instant::now();
    let ticks = Arc::new(Mutex::new(0));
    let ticks_c = Arc::clone(&ticks);
    interval(Duration::from_millis(1), local.spawner())
      .take(5)
      .subscribe(move |_| (*ticks_c.lock().unwrap()) += 1);
    local.run();
    assert_eq!(*ticks.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }
}
