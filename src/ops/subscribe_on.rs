use crate::{
  prelude::*,
  scheduler::{OnceTask, Scheduler, SubscribeReturn, TaskHandle},
};

#[derive(Clone)]
pub struct SubscribeOnOP<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

impl<S, Item, Err, O, SD> ObservableImpl<Item, Err, O> for SubscribeOnOP<S, SD>
where
  O: Observer<Item, Err>,
  S: ObservableImpl<Item, Err, O>,
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
  S: ObservableImpl<Item, Err, O>,
  O: Observer<Item, Err>,
{
  SubscribeReturn::new(source.actual_subscribe(observer))
}

impl<S, Item, Err, SD> Observable<Item, Err> for SubscribeOnOP<S, SD> where
  S: Observable<Item, Err>
{
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
  use crate::prelude::*;
  use crate::rc::{MutArc, RcDerefMut};
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::{Duration, Instant};

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn thread_pool() {
    let scheduler = SharedScheduler;
    let res = Arc::new(Mutex::new(vec![]));
    let c_res = res.clone();
    let thread = Arc::new(Mutex::new(vec![]));
    let c_thread = thread.clone();
    observable::from_iter(1..5)
      .subscribe_on(scheduler)
      .subscribe(move |v| {
        res.lock().unwrap().push(v);
        let handle = thread::current();
        thread.lock().unwrap().push(handle.id());
      });

    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    assert_eq!(*c_res.lock().unwrap(), (1..5).collect::<Vec<_>>());
    assert_ne!(c_thread.lock().unwrap()[0], thread::current().id());
  }

  #[tokio::test]
  async fn pool_unsubscribe() {
    let emitted = Arc::new(Mutex::new(0));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .delay_threads(Duration::from_millis(10), SharedScheduler)
      .subscribe_on(SharedScheduler)
      .subscribe(move |_| {
        eprintln!("accept value");
        *emitted.lock().unwrap() += 1;
      })
      .unsubscribe();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(*c_emitted.lock().unwrap(), 0);
  }

  #[tokio::test]
  async fn parallel_subscribe_on() {
    let pool_scheduler = SharedScheduler;
    let (o, status) = from_iter(0..2)
      .flat_map_threads(move |v| of(v).subscribe_on(pool_scheduler))
      .complete_status();

    let hit_times = MutArc::own(vec![]);
    let c_hit_times = hit_times.clone();
    let now = Instant::now();
    o.subscribe(move |_| c_hit_times.rc_deref_mut().push(Instant::now()));

    status.wait_completed().await;

    let finished_in_same_second = hit_times
      .rc_deref_mut()
      .iter()
      .all(|at| at.duration_since(now).as_secs() < 2);
    assert!(finished_in_same_second);
  }
}
