use crate::{
  prelude::*,
  scheduler::{OnceTask, Scheduler, SubscribeReturn, TaskHandle},
};

#[derive(Clone)]
pub struct SubscribeOnOP<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

impl<S, Item, Err, O, SD> Observable<Item, Err, O> for SubscribeOnOP<S, SD>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, O>,
  SD: Scheduler<OnceTask<(S, O), SubscribeReturn<S::Unsub>>>,
  S::Unsub: 'static,
{
  type Unsub = TaskHandle<SubscribeReturn<S::Unsub>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, scheduler } = self;
    scheduler.schedule(OnceTask::new(subscribe_task, (source, observer)), None)
  }
}

fn subscribe_task<S, O, Item, Err>(
  (source, observer): (S, O),
) -> SubscribeReturn<S::Unsub>
where
  S: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  SubscribeReturn::new(source.actual_subscribe(observer))
}

impl<S, Item, Err, SD> ObservableExt<Item, Err> for SubscribeOnOP<S, SD> where
  S: ObservableExt<Item, Err>
{
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
  use crate::ops::complete_status::CompleteStatus;
  use crate::prelude::*;
  use crate::rc::{MutArc, RcDerefMut};
  use futures::executor::ThreadPool;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::{Duration, Instant};

  #[test]
  fn thread_pool() {
    let pool = ThreadPool::new().unwrap();
    let res = Arc::new(Mutex::new(vec![]));
    let c_res = res.clone();
    let thread = Arc::new(Mutex::new(vec![]));
    let c_thread = thread.clone();
    observable::from_iter(1..5)
      .subscribe_on(pool)
      .subscribe(move |v| {
        res.lock().unwrap().push(v);
        let handle = thread::current();
        thread.lock().unwrap().push(handle.id());
      });

    thread::sleep(std::time::Duration::from_millis(1));
    assert_eq!(*c_res.lock().unwrap(), (1..5).collect::<Vec<_>>());
    assert_ne!(c_thread.lock().unwrap()[0], thread::current().id());
  }

  #[test]
  fn pool_unsubscribe() {
    let pool = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(0));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .delay_threads(Duration::from_millis(10), pool.clone())
      .subscribe_on(pool)
      .subscribe(move |_| {
        eprintln!("accept value");
        *emitted.lock().unwrap() += 1;
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(*c_emitted.lock().unwrap(), 0);
  }

  #[test]
  fn parallel_subscribe_on() {
    let pool_scheduler = FuturesThreadPoolScheduler::new().unwrap();
    let (o, status) = from_iter(0..2)
      .flat_map_threads(move |v| {
        of(v)
          .tap(|_| thread::sleep(Duration::from_secs(1)))
          .subscribe_on(pool_scheduler.clone())
      })
      .complete_status();

    let hit_times = MutArc::own(vec![]);
    let c_hit_times = hit_times.clone();
    let now = Instant::now();
    o.subscribe(move |_| c_hit_times.rc_deref_mut().push(Instant::now()));

    CompleteStatus::wait_for_end(status);

    let finished_in_same_second = hit_times
      .rc_deref_mut()
      .iter()
      .all(|at| at.duration_since(now).as_secs() < 2);
    assert!(finished_in_same_second);
  }
}
