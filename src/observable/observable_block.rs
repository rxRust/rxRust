#![cfg(test)]
use crate::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ObserverBlock<N, Item> {
  next: N,
  is_stopped: Arc<AtomicBool>,
  _marker: TypeHint<*const Item>,
}

impl<Item, N> ObserverBlock<N, Item> {
  #[inline(always)]
  pub fn new(next: N) -> Self {
    ObserverBlock {
      next,
      is_stopped: Arc::new(AtomicBool::new(false)),
      _marker: TypeHint::new(),
    }
  }
}

impl<Item, N> Observer for ObserverBlock<N, Item>
where
  N: FnMut(Item),
{
  type Item = Item;
  type Err = ();
  #[inline(always)]
  fn next(&mut self, value: Self::Item) { (self.next)(value); }

  fn error(&mut self, _err: ()) {
    self.is_stopped.store(true, Ordering::Relaxed);
  }

  fn complete(&mut self) { self.is_stopped.store(true, Ordering::Relaxed) }
}

pub trait SubscribeBlocking<'a, N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable that will block the subscribing
  /// thread useful for testing and last resort blocking in token scenarios.
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
  fn subscribe_blocking(self, next: N) -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N> SubscribeBlocking<'a, N> for Shared<S>
where
  S: SharedObservable<Err = ()>,
  N: FnMut(S::Item) + Send + Sync + 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_blocking(self, next: N) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let stopped = Arc::new(AtomicBool::new(false));
    let stopped_c = Arc::clone(&stopped);
    let subscriber = Subscriber::shared(ObserverBlock {
      next,
      is_stopped: stopped,
      _marker: TypeHint::new(),
    });
    let sub = SubscriptionWrapper(self.actual_subscribe(subscriber));
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
    interval
      .clone()
      .subscribe_blocking(move |_| *first_clone.lock().unwrap() += 1);
    assert_eq!(*first.lock().unwrap(), 5);

    let second = Arc::new(Mutex::new(0));
    let second_clone = Arc::clone(&second);
    interval.subscribe_blocking(move |_| *second_clone.lock().unwrap() += 1);
    assert_eq!(*first.lock().unwrap(), 5);
    assert_eq!(*second.lock().unwrap(), 5);

    assert!(stamp.elapsed() > Duration::from_millis(10));
  }
}
