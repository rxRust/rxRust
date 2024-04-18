use crate::{
  prelude::*,
  rc::{MutArc, RcDerefMut},
};
use std::time::Duration;

/// Config to define leading and trailing behavior for throttle
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ThrottleEdge {
  tailing: bool,
  leading: bool,
}

#[derive(Clone)]
pub struct ThrottleOp<S, SD, F> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
  pub(crate) duration_selector: F,
  pub(crate) edge: ThrottleEdge,
}

impl ThrottleEdge {
  #[inline]
  pub fn leading() -> Self {
    Self { tailing: false, leading: true }
  }

  #[inline]
  pub fn tailing() -> Self {
    Self { tailing: true, leading: false }
  }

  #[inline]
  pub fn all() -> Self {
    Self { tailing: true, leading: true }
  }
}

impl<Item, Err, O, S, SD, F> Observable<Item, Err, O> for ThrottleOp<S, SD, F>
where
  Item: Clone,
  O: Observer<Item, Err>,
  S: Observable<Item, Err, ThrottleObserver<O, SD, Item, F>>,
  F: FnMut(&Item) -> Duration,
  ThrottleObserver<O, SD, Item, F>: Observer<Item, Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self {
      source,
      scheduler,
      duration_selector,
      edge,
    } = self;

    source.actual_subscribe(ThrottleObserver {
      observer: MutArc::own(Some(observer)),
      edge,
      duration_selector,
      trailing_value: MutArc::own(None),
      task_handler: TaskHandle::value_handle(NormalReturn::new(())),
      scheduler,
    })
  }
}

impl<Item, Err, S, SD, F> ObservableExt<Item, Err> for ThrottleOp<S, SD, F> where
  S: ObservableExt<Item, Err>
{
}

pub struct ThrottleObserver<O, SD, Item, F> {
  scheduler: SD,
  observer: MutArc<Option<O>>,
  edge: ThrottleEdge,
  duration_selector: F,
  trailing_value: MutArc<Option<Item>>,
  task_handler: TaskHandle<NormalReturn<()>>,
}

impl<Item, Err, O, SD, F> Observer<Item, Err>
  for ThrottleObserver<O, SD, Item, F>
where
  Item: Clone,
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> Duration,
  SD: Scheduler<
    OnceTask<(MutArc<Option<O>>, MutArc<Option<Item>>), NormalReturn<()>>,
  >,
{
  fn next(&mut self, value: Item) {
    if self.edge.leading || self.edge.tailing {
      if self.edge.tailing {
        *self.trailing_value.rc_deref_mut() = Some(value.clone());
      }
      if self.task_handler.is_closed() {
        let delay = (self.duration_selector)(&value);
        if self.edge.leading {
          self.observer.next(value)
        }
        let task = OnceTask::new(
          throttle_task,
          (self.observer.clone(), self.trailing_value.clone()),
        );
        self.task_handler = self.scheduler.schedule(task, Some(delay));
      }
    }
  }

  fn error(self, err: Err) {
    self.observer.error(err);
    self.task_handler.unsubscribe();
  }

  fn complete(mut self) {
    if let Some(value) = self.trailing_value.rc_deref_mut().take() {
      self.observer.next(value);
    }
    self.task_handler.unsubscribe();
    self.observer.complete();
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

fn throttle_task<O, Item, Err>(
  (mut observer, trailing_value): (MutArc<Option<O>>, MutArc<Option<Item>>),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  if let Some(v) = trailing_value.rc_deref_mut().take() {
    observer.next(v);
  }
  NormalReturn::new(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::rc::{MutRc, RcDeref};

  #[test]
  fn smoke() {
    let x = MutRc::own(vec![]);
    let mut pool = FuturesLocalSchedulerPool::new();
    let scheduler = pool.spawner();

    let throttle_subscribe = |edge| {
      let x = x.clone();
      observable::interval(Duration::from_millis(5), scheduler.clone())
        .take(5)
        .throttle(
          |val| -> Duration {
            if val % 2 == 0 {
              Duration::from_millis(7)
            } else {
              Duration::from_millis(5)
            }
          },
          edge,
          scheduler.clone(),
        )
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };

    // tailing throttle
    let sub = throttle_subscribe(ThrottleEdge::tailing());
    pool.run();
    sub.unsubscribe();
    assert_eq!(&*x.rc_deref(), &[1, 3, 4]);

    // leading throttle
    x.rc_deref_mut().clear();
    throttle_subscribe(ThrottleEdge::leading());
    pool.run();
    assert_eq!(&*x.rc_deref(), &[0, 2, 4]);
  }

  #[test]
  fn smoke_for_throttle_time() {
    let x = MutRc::own(vec![]);
    let mut pool = FuturesLocalSchedulerPool::new();
    let scheduler = pool.spawner();

    let throttle_time_subscribe = |edge| {
      let x = x.clone();
      observable::interval(Duration::from_millis(50), scheduler.clone())
        .take(5)
        .throttle_time(Duration::from_millis(115), edge, scheduler)
        .subscribe(move |v| x.rc_deref_mut().push(v));
    };

    // tailing throttle
    (throttle_time_subscribe.clone())(ThrottleEdge::tailing());
    pool.run();

    assert_eq!(&*x.rc_deref(), &[2, 4]);

    // leading throttle
    x.rc_deref_mut().clear();
    throttle_time_subscribe(ThrottleEdge::leading());
    pool.run();

    assert_eq!(&*x.rc_deref(), &[0, 3]);
  }
}
