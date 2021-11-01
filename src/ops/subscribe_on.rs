use crate::prelude::*;
use crate::scheduler::SharedScheduler;

#[derive(Clone)]
pub struct SubscribeOnOP<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(SubscribeOnOP, S, SD);

impl<'a, S, SD> SharedObservable for SubscribeOnOP<S, SD>
where
  S: SharedObservable + Send + 'static,
  SD: SharedScheduler + Send + 'static,
  S::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let source = self.source;
    let subscription = SharedSubscription::default();
    let c_subscription = subscription.clone();
    let handle = self.scheduler.schedule(
      move |_| c_subscription.add(source.actual_subscribe(observer)),
      None,
      (),
    );
    subscription.add(handle);
    subscription
  }
}

impl<S, SD> LocalObservable<'static> for SubscribeOnOP<S, SD>
where
  S: LocalObservable<'static> + 'static,
  SD: LocalScheduler,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    let source = self.source;

    let subscription = LocalSubscription::default();
    let c_subscription = subscription.clone();
    let handle = self.scheduler.schedule(
      move |_| c_subscription.add(source.actual_subscribe(observer)),
      None,
      (),
    );
    subscription.add(handle);
    subscription
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::ThreadPool;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;

  #[test]
  fn thread_pool() {
    let pool = ThreadPool::new().unwrap();
    let res = Arc::new(Mutex::new(vec![]));
    let c_res = res.clone();
    let thread = Arc::new(Mutex::new(vec![]));
    let c_thread = thread.clone();
    observable::from_iter(1..5)
      .subscribe_on(pool)
      .into_shared()
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
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .subscribe_on(pool.clone())
      .delay(Duration::from_millis(10), pool)
      .into_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
