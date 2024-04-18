use crate::{
  prelude::*,
  rc::{MutArc, RcDerefMut},
};
use std::time::Duration;
#[derive(Clone)]
pub struct DebounceOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
  pub(crate) duration: Duration,
}

type RcHandler = MutArc<Option<TaskHandle<NormalReturn<()>>>>;
impl<Item, Err, O, S, SD> Observable<Item, Err, O> for DebounceOp<S, SD>
where
  S: Observable<Item, Err, DebounceObserver<O, SD, Item>>,
  SD: Scheduler<
    OnceTask<(MutArc<Option<O>>, MutArc<Option<Item>>), NormalReturn<()>>,
  >,
  O: Observer<Item, Err>,
{
  type Unsub = ZipSubscription<S::Unsub, RcHandler>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, scheduler, duration } = self;
    let task_handler = MutArc::own(None);
    let observer = DebounceObserver {
      observer: MutArc::own(Some(observer)),
      delay: duration,
      scheduler,
      trailing_value: MutArc::own(None),
      task_handler: task_handler.clone(),
    };
    let u = source.actual_subscribe(observer);
    ZipSubscription::new(u, task_handler)
  }
}

impl<Item, Err, S, SD> ObservableExt<Item, Err> for DebounceOp<S, SD> where
  S: ObservableExt<Item, Err>
{
}

pub struct DebounceObserver<O, SD, Item> {
  observer: MutArc<Option<O>>,
  scheduler: SD,
  delay: Duration,
  trailing_value: MutArc<Option<Item>>,
  task_handler: RcHandler,
}

fn debounce_task<O, Item, Err>(
  (mut observer, value): (MutArc<Option<O>>, MutArc<Option<Item>>),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  if let Some(value) = value.rc_deref_mut().take() {
    observer.next(value);
  }
  NormalReturn::new(())
}

impl<Item, Err, O, SD> Observer<Item, Err> for DebounceObserver<O, SD, Item>
where
  O: Observer<Item, Err>,
  SD: Scheduler<
    OnceTask<(MutArc<Option<O>>, MutArc<Option<Item>>), NormalReturn<()>>,
  >,
{
  fn next(&mut self, value: Item) {
    *self.trailing_value.rc_deref_mut() = Some(value);
    let observer = self.observer.clone();
    let tail_value = self.trailing_value.clone();
    let task = OnceTask::new(debounce_task, (observer, tail_value));
    if let Some(handler) = self.task_handler.rc_deref_mut().take() {
      handler.unsubscribe()
    }
    let handler = self.scheduler.schedule(task, Some(self.delay));
    *self.task_handler.rc_deref_mut() = Some(handler);
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err);
  }

  fn complete(mut self) {
    if let Some(value) = self.trailing_value.rc_deref_mut().take() {
      self.observer.next(value);
    }
    self.observer.complete();
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::rc::{MutRc, RcDeref};
  use futures::executor::LocalPool;
  #[test]
  fn smoke_last() {
    let x = MutRc::own(vec![]);
    let mut pool = LocalPool::new();
    let interval =
      observable::interval(Duration::from_millis(20), pool.spawner());
    let spawner = pool.spawner();
    let debounce_subscribe = || {
      let x = x.clone();
      interval
        .clone()
        .take(10)
        .debounce(Duration::from_millis(30), spawner.clone())
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };
    let sub = debounce_subscribe();
    pool.run();
    sub.unsubscribe();
    assert_eq!(&*x.rc_deref(), &[9]);
  }

  #[test]
  fn smoke_every() {
    let x = MutRc::own(vec![]);
    let mut pool = LocalPool::new();
    let interval =
      observable::interval(Duration::from_millis(30), pool.spawner());
    let spawner = pool.spawner();
    let debounce_subscribe = || {
      let x = x.clone();
      interval
        .clone()
        .take(10)
        .debounce(Duration::from_millis(20), spawner.clone())
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };
    let sub = debounce_subscribe();
    pool.run();
    sub.unsubscribe();
    assert_eq!(&*x.rc_deref(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
  }
}
