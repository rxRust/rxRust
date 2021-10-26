#![cfg(test)]
use crate::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ObserverBlockAll<N, E, C, Item, Err> {
  next: N,
  error: E,
  complete: C,
  is_stopped: Arc<AtomicBool>,
  _marker: TypeHint<(*const Item, *const Err)>,
}

impl<Item, Err, N, E, C> ObserverBlockAll<N, E, C, Item, Err> {
  #[inline(always)]
  pub fn new(next: N, error: E, complete: C) -> Self {
    ObserverBlockAll {
      next,
      error,
      complete,
      is_stopped: Arc::new(AtomicBool::new(false)),
      _marker: TypeHint::new(),
    }
  }
}

impl<Item, Err, N, E, C> Observer for ObserverBlockAll<N, E, C, Item, Err>
where
  C: FnMut(),
  N: FnMut(Item),
  E: FnMut(Err),
{
  type Item = Item;
  type Err = Err;
  #[inline(always)]
  fn next(&mut self, value: Self::Item) { (self.next)(value); }

  fn error(&mut self, err: Self::Err) {
    (self.error)(err);
    self.is_stopped.store(true, Ordering::Relaxed);
  }

  fn complete(&mut self) {
    (self.complete)();
    self.is_stopped.store(true, Ordering::Relaxed);
  }
}

pub trait SubscribeBlockingAll<'a, N, E, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable that will block the subscribing
  /// thread; useful for testing and last resort blocking in token scenarios.
  ///
  /// Will return a SubscriptionWrapper only after upstream completion.
  ///
  /// Should preferably not be used in production because of both its blocking
  /// nature, as well as its implementation by an arbitrarily chosen 1ms
  /// thread sleep which goes against reactive programming philosophy.
  ///
  /// Use with caution, will block forever if the upstream never completes or
  /// errors out.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  fn subscribe_blocking_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N, E, C> SubscribeBlockingAll<'a, N, E, C> for Shared<S>
where
  S: SharedObservable,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
  C: FnMut() + Send + Sync + 'static,
  S::Err: 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_blocking_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let stopped = Arc::new(AtomicBool::new(false));
    let stopped_c = Arc::clone(&stopped);
    let subscriber = Subscriber::shared(ObserverBlockAll {
      next,
      error,
      complete,
      is_stopped: stopped,
      _marker: TypeHint::new(),
    });
    let sub = SubscriptionWrapper(self.0.actual_subscribe(subscriber));
    while !stopped_c.load(Ordering::Relaxed) {
      std::thread::sleep(Duration::from_millis(1))
    }
    sub
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::ThreadPool;
  use std::sync::{Arc, Mutex};
  use std::time::{Duration, Instant};

  #[test]
  fn blocks_shared() {
    let pool = ThreadPool::new().unwrap();
    let stamp = Instant::now();
    let interval = observable::interval(Duration::from_millis(1), pool)
      .take(5)
      .into_shared();

    let first = Arc::new(Mutex::new(0));
    let first_clone = Arc::clone(&first);
    interval.clone().subscribe_blocking_all(
      move |_| *first_clone.lock().unwrap() += 1,
      |_| {},
      || {},
    );
    assert_eq!(*first.lock().unwrap(), 5);

    let second = Arc::new(Mutex::new(0));
    let second_clone = Arc::clone(&second);
    interval.subscribe_blocking_all(
      move |_| *second_clone.lock().unwrap() += 1,
      |_| {},
      || {},
    );

    assert_eq!(*first.lock().unwrap(), 5);
    assert_eq!(*second.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(10));
  }
}
