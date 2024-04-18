use crate::{
  prelude::*,
  rc::{MutRc, RcDeref, RcDerefMut},
};
use std::{collections::VecDeque, convert::Infallible};

#[derive(Clone)]
pub struct FakeClock(MutRc<InnerTimer>);

struct InnerTimer {
  current: Instant,
  vec: VecDeque<TimerObserver>,
}

enum TimerObserver {
  Interval {
    at: Instant,
    seq: usize,
    duration: Duration,
    task: Box<dyn Publisher<usize, Infallible>>,
  },
  Timer {
    at: Instant,
    task: Box<dyn Publisher<Instant, Infallible>>,
  },
}

impl TimerObserver {
  fn at(&self) -> Instant {
    match self {
      TimerObserver::Interval { at, .. } => *at,
      TimerObserver::Timer { at, .. } => *at,
    }
  }
}

impl FakeClock {
  pub fn current_time(&self) -> Instant {
    self.0.rc_deref().current
  }

  pub fn interval(&self, duration: Duration) -> IntervalObservable {
    IntervalObservable { duration, timer: self.clone() }
  }

  pub fn delay(&self, delay: Duration) -> DelayObservable {
    DelayObservable { delay, timer: self.clone() }
  }

  pub fn advance(&self, duration: Duration) {
    let to = self.current_time() + duration;

    while let Some(task) = self.turn_to_first_expired(to) {
      match task {
        TimerObserver::Interval { seq, duration, mut task, at } => {
          task.p_next(seq);
          if !task.p_is_closed() {
            let tasks = &mut self.0.rc_deref_mut().vec;
            order_insert(
              tasks,
              TimerObserver::Interval {
                seq: seq + 1,
                task,
                at: at + duration,
                duration,
              },
            );
          }
        }
        TimerObserver::Timer { mut task, .. } => {
          task.p_next(to);
          task.p_complete();
        }
      }
    }

    self.0.rc_deref_mut().current = to;
  }

  fn insert(&mut self, task: TimerObserver) {
    let tasks = &mut self.0.rc_deref_mut().vec;
    order_insert(tasks, task);
  }

  fn turn_to_first_expired(&self, to: Instant) -> Option<TimerObserver> {
    let first_expired = self.0.rc_deref().vec.front().map(|t| to > t.at())?;
    first_expired.then(|| {
      let mut inner = self.0.rc_deref_mut();
      let task = inner.vec.pop_front().unwrap();
      inner.current = task.at();
      task
    })
  }
}

fn order_insert(tasks: &mut VecDeque<TimerObserver>, task: TimerObserver) {
  let at = task.at();
  let position = tasks
    .make_contiguous()
    .binary_search_by(|t| t.at().cmp(&at));
  let position = match position {
    Ok(p) => p,
    Err(p) => p,
  };
  tasks.insert(position, task);
}

impl Default for FakeClock {
  fn default() -> Self {
    let inner = InnerTimer {
      current: Instant::now(),
      vec: Default::default(),
    };
    FakeClock(MutRc::own(inner))
  }
}

#[derive(Clone)]
pub struct IntervalObservable {
  duration: Duration,
  timer: FakeClock,
}

#[derive(Clone)]
pub struct DelayObservable {
  delay: Duration,
  timer: FakeClock,
}

impl<O> Observable<usize, Infallible, O> for IntervalObservable
where
  O: Observer<usize, Infallible> + 'static,
{
  type Unsub = Subscriber<O>;

  fn actual_subscribe(mut self, observer: O) -> Self::Unsub {
    let subscriber = Subscriber::new(Some(observer));
    let task = TimerObserver::Interval {
      at: self.timer.current_time() + self.duration,
      duration: self.duration,
      seq: 0,
      task: Box::new(subscriber.clone()),
    };
    self.timer.insert(task);
    subscriber
  }
}

impl ObservableExt<usize, Infallible> for IntervalObservable {}

impl<O> Observable<Instant, Infallible, O> for DelayObservable
where
  O: Observer<Instant, Infallible> + 'static,
{
  type Unsub = Subscriber<O>;

  fn actual_subscribe(mut self, observer: O) -> Self::Unsub {
    let subscriber = Subscriber::new(Some(observer));
    let task = TimerObserver::Timer {
      at: self.timer.current_time() + self.delay,
      task: Box::new(subscriber.clone()),
    };
    self.timer.insert(task);
    subscriber
  }
}

impl ObservableExt<Instant, Infallible> for DelayObservable {}
