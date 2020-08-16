use crate::prelude::*;

use futures::{
  future::RemoteHandle,
  prelude::*,
  task::{LocalSpawn, LocalSpawnExt, Spawn, SpawnExt},
};
use futures_timer::Delay;
use std::time::Duration;

/// A Scheduler is an object to order task and schedule their execution.
pub trait SharedScheduler {
  fn schedule<T: Send + 'static>(
    &mut self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SharedSubscription;
}

pub trait LocalScheduler {
  fn schedule<T: 'static>(
    &mut self,
    task: impl FnOnce(LocalSubscription, T) + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> LocalSubscription;
}

impl<S: Spawn> SharedScheduler for S {
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
    let handle = self
      .spawn_with_handle(f)
      .expect("spawn task to thread pool failed.");
    subscription.add(SpawnHandle::new(handle));
    subscription
  }
}

impl<L: LocalSpawn> LocalScheduler for L {
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
    let handle = self
      .spawn_local_with_handle(f)
      .expect("spawn task to thread pool failed.");
    subscription.add(SpawnHandle::new(handle));
    subscription
  }
}

pub struct SpawnHandle<T>(Option<RemoteHandle<T>>);

impl<T> SpawnHandle<T> {
  #[inline(always)]
  pub fn new(handle: RemoteHandle<T>) -> Self { SpawnHandle(Some(handle)) }
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
  use std::f32;
  use test::Bencher;

  #[bench]
  fn pool(b: &mut Bencher) {
    let pool = ThreadPool::new().unwrap();
    b.iter(|| {
      observable::from_iter(0..100)
        .observe_on(pool.clone())
        .to_shared()
        .subscribe(move |v| {
          (0..1000).fold((v as f32).sqrt(), |acc, _| acc.sqrt());
        })
    })
  }

  #[bench]
  fn local_thread(b: &mut Bencher) {
    let local = LocalPool::new();
    let spawner = local.spawner();
    b.iter(|| {
      observable::from_iter(0..100)
        .observe_on(spawner.clone())
        .subscribe(move |v| {
          (0..1000).fold((v as f32).sqrt(), |acc, _| acc.sqrt());
        })
    })
  }
}
