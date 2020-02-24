use crate::prelude::*;
use crate::scheduler::Scheduler;
use observable::observable_proxy_impl;

#[derive(Clone)]
pub struct SubscribeOnOP<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(SubscribeOnOP, S, SD);

impl<'a, S, SD> SharedObservable for SubscribeOnOP<S, SD>
where
  S: SharedObservable + Send + 'static,
  SD: Scheduler + Send + 'static,
  S::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let source = self.source;
    self.scheduler.schedule(
      move |mut subscription, _| {
        subscription.add(source.actual_subscribe(subscriber))
      },
      None,
      (),
    )
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use crate::scheduler::Schedulers;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;

  #[test]
  fn new_thread() {
    let res = Arc::new(Mutex::new(vec![]));
    let c_res = res.clone();
    let thread = Arc::new(Mutex::new(vec![]));
    let c_thread = thread.clone();
    observable::from_iter(1..5)
      .subscribe_on(Schedulers::NewThread)
      .to_shared()
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
  fn pool_unsubscribe() { unsubscribe_scheduler(Schedulers::ThreadPool) }

  #[test]
  fn new_thread_unsubscribe() { unsubscribe_scheduler(Schedulers::NewThread) }

  // #[test]
  // fn sync_unsubscribe() { unsubscribe_scheduler(Schedulers::Sync) }

  fn unsubscribe_scheduler(scheduler: Schedulers) {
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .subscribe_on(scheduler)
      .delay(Duration::from_millis(10))
      .to_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
