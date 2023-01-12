use crate::prelude::*;
use std::time::Duration;

#[derive(Clone)]
pub struct DelayOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

impl<Item, Err, O, S, SD> Observable<Item, Err, O> for DelayOp<S, SD>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, O>,
  S::Unsub: 'static,
  SD: Scheduler<OnceTask<(S, O), SubscribeReturn<S::Unsub>>>,
{
  type Unsub = TaskHandle<SubscribeReturn<S::Unsub>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let task = OnceTask::new(subscribe_task, (self.source, observer));
    self.scheduler.schedule(task, Some(self.delay))
  }
}

impl<Item, Err, S, SD> ObservableExt<Item, Err> for DelayOp<S, SD> where
  S: ObservableExt<Item, Err>
{
}

pub fn subscribe_task<S, O, Item, Err>(
  (source, observer): (S, O),
) -> SubscribeReturn<S::Unsub>
where
  S: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  SubscribeReturn::new(source.actual_subscribe(observer))
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::LocalPool;
  use std::{cell::RefCell, rc::Rc, time::Instant};

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared_smoke() {
    use futures::executor::ThreadPool;
    use std::sync::{Arc, Mutex};

    use crate::ops::complete_status::CompleteStatus;

    let value = Arc::new(Mutex::new(0));
    let c_value = value.clone();
    let pool = ThreadPool::new().unwrap();
    let stamp = Instant::now();
    let (o, status) = observable::of(1)
      .delay(Duration::from_millis(50), pool)
      .complete_status();

    o.subscribe(move |v| {
      *value.lock().unwrap() = v;
    });
    CompleteStatus::wait_for_end(status);

    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*c_value.lock().unwrap(), 1);
  }

  #[test]
  fn local_smoke() {
    let value = Rc::new(RefCell::new(0));
    let c_value = value.clone();
    let mut pool = LocalPool::new();
    observable::of(1)
      .delay(Duration::from_millis(50), pool.spawner())
      .subscribe(move |v| {
        *c_value.borrow_mut() = v;
      });
    assert_eq!(*value.borrow(), 0);
    let stamp = Instant::now();
    pool.run();
    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*value.borrow(), 1);
  }
}
