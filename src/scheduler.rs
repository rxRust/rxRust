use crate::prelude::*;
use futures::{
  future::RemoteHandle,
  task::{LocalSpawn, LocalSpawnExt, Spawn, SpawnExt},
  FutureExt,
};
use futures_timer::Delay;
use std::future::Future;
use std::time::Duration;

/// A Scheduler is an object to order task and schedule their execution.
pub trait SharedScheduler {
  fn spawn<Fut>(&self, future: Fut, subscription: &mut SharedSubscription)
  where
    Fut: Future<Output = ()> + Send + 'static;

  fn schedule<T: Send + 'static>(
    &mut self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SharedSubscription {
    let mut subscription = SharedSubscription::default();
    let c_subscription = subscription.clone();
    let delay = delay.unwrap_or_default();
    let f = Delay::new(delay).inspect(|_| task(c_subscription, state));
    self.spawn(f, &mut subscription);
    subscription
  }
}

pub trait LocalScheduler {
  fn spawn<Fut>(&self, future: Fut, subscription: &mut LocalSubscription)
  where
    Fut: Future<Output = ()> + 'static;

  fn schedule<T: 'static>(
    &mut self,
    task: impl FnOnce(LocalSubscription, T) + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> LocalSubscription {
    let mut subscription = LocalSubscription::default();
    let c_subscription = subscription.clone();
    let delay = delay.unwrap_or_default();
    let f = Delay::new(delay).inspect(|_| task(c_subscription, state));
    self.spawn(f, &mut subscription);

    subscription
  }
}

impl<S: Spawn> SharedScheduler for S {
  fn spawn<Fut>(&self, future: Fut, subscription: &mut SharedSubscription)
  where
    Fut: Future<Output = ()> + Send + 'static,
  {
    let handle = self
      .spawn_with_handle(future)
      .expect("spawn task to thread pool failed.");
    subscription.add(SpawnHandle::new(handle))
  }
}

impl<L: LocalSpawn> LocalScheduler for L {
  fn spawn<Fut>(&self, future: Fut, subscription: &mut LocalSubscription)
  where
    Fut: Future<Output = ()> + 'static,
  {
    let handle = self
      .spawn_local_with_handle(future)
      .expect("spawn task to thread pool failed.");
    subscription.add(SpawnHandle::new(handle))
  }
}

struct SpawnHandle<T>(Option<RemoteHandle<T>>);

impl<T> SpawnHandle<T> {
  #[inline(always)]
  fn new(handle: RemoteHandle<T>) -> Self { SpawnHandle(Some(handle)) }
}

impl<T> SubscriptionLike for SpawnHandle<T> {
  #[inline(always)]
  fn unsubscribe(&mut self) { self.0.take(); }
  #[inline(always)]
  fn is_closed(&self) -> bool { self.0.is_none() }
  #[inline(always)]
  fn inner_addr(&self) -> *const () { ((&self.0) as *const _) as *const () }
}

impl<T> Drop for SpawnHandle<T> {
  fn drop(&mut self) {
    if self.0.is_some() {
      self.0.take().unwrap().forget()
    }
  }
}

#[cfg(test)]
mod test {
  extern crate test;
  use crate::prelude::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};
  use test::Bencher;

  #[bench]
  fn pool(b: &mut Bencher) {
    let sum = Arc::new(Mutex::new(0.));
    b.iter(|| {
      let sum = sum.clone();
      let pool = ThreadPool::new().unwrap();
      observable::from_iter(0..1000)
        .observe_on(pool)
        .to_shared()
        .subscribe(move |v| {
          *sum.lock().unwrap() =
            (0..1000).fold((v as f32).sqrt(), |acc, _| acc.sqrt());
        })
    })
  }

  #[bench]
  fn local_thread(b: &mut Bencher) {
    let sum = Arc::new(Mutex::new(0.));
    b.iter(|| {
      let mut local = LocalPool::new();
      let sum = sum.clone();
      observable::from_iter(0..1000)
        .observe_on(local.spawner())
        .subscribe(move |v| {
          *sum.lock().unwrap() =
            (0..1000).fold((v as f32).sqrt(), |acc, _| acc.sqrt());
        });

      local.run();
    })
  }
}
