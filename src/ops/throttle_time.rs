use crate::prelude::*;
use observable::observable_proxy_impl;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
  time::Duration,
};

/// Config to define leading and trailing behavior for throttle
#[derive(PartialEq, Clone, Copy)]
pub enum ThrottleEdge {
  Tailing,
  Leading,
}

#[derive(Clone)]
pub struct ThrottleTimeOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
  pub(crate) duration: Duration,
  pub(crate) edge: ThrottleEdge,
}

observable_proxy_impl!(ThrottleTimeOp, S, SD);

impl<Item, Err, S, SD, Unsub> LocalObservable<'static> for ThrottleTimeOp<S, SD>
where
  S: for<'r> LocalObservable<'r, Item = Item, Err = Err, Unsub = Unsub>,
  Unsub: SubscriptionLike + 'static,
  Item: Clone + 'static,
  SD: LocalScheduler + 'static,
{
  type Unsub = Unsub;

  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'static>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let Self {
      source,
      duration,
      edge,
      scheduler,
    } = self;

    source.actual_subscribe(Subscriber {
      observer: LocalThrottleObserver(Rc::new(RefCell::new(
        ThrottleObserver {
          observer: subscriber.observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscriber.subscription.clone(),
          scheduler,
        },
      ))),
      subscription: subscriber.subscription,
    })
  }
}

impl<S, SD> SharedObservable for ThrottleTimeOp<S, SD>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
  SD: SharedScheduler + Send + 'static,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> S::Unsub {
    let Self {
      source,
      duration,
      edge,
      scheduler,
    } = self;
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    source.actual_subscribe(Subscriber {
      observer: SharedThrottleObserver(Arc::new(Mutex::new(
        ThrottleObserver {
          observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscription.clone(),
          scheduler,
        },
      ))),
      subscription,
    })
  }
}

struct ThrottleObserver<O, S, Item, Sub> {
  scheduler: S,
  observer: O,
  edge: ThrottleEdge,
  delay: Duration,
  trailing_value: Option<Item>,
  throttled: Option<Sub>,
  subscription: Sub,
}

struct SharedThrottleObserver<O, S, Item>(
  Arc<Mutex<ThrottleObserver<O, S, Item, SharedSubscription>>>,
);

struct LocalThrottleObserver<O, S, Item>(
  Rc<RefCell<ThrottleObserver<O, S, Item, LocalSubscription>>>,
);

macro impl_throttle_observer($item: ident, $err: ident, $($path: ident).*) {
  fn next(&mut self, value: $item) {
    let mut inner = self.0.$($path()).*;
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let c_inner = self.0.clone();
      let delay = inner.delay;
      let subscription = inner.scheduler.schedule(
        move |_, _| {
          let mut inner = c_inner.$($path()).*;
          if let Some(v) = inner.trailing_value.take() {
            inner.observer.next(v);
          }
          if let Some(mut throttled) = inner.throttled.take() {
            throttled.unsubscribe();
            inner.subscription.remove(&throttled);
          }
        },
        Some(delay),
        (),
      );
      inner.subscription.add(subscription.clone());
      inner.throttled = Some(subscription);
      if inner.edge == ThrottleEdge::Leading {
        inner.observer.next(value);
      }
    }
  }

  fn error(&mut self, err: $err) {
    let mut inner = self.0.$($path()).*;
    inner.observer.error(err)
  }

  fn complete(&mut self) {
    let mut inner = self.0.$($path()).*;
    if let Some(value) = inner.trailing_value.take() {
      inner.observer.next(value);
    }
    inner.observer.complete();
  }
}

impl<O, S, Item, Err> Observer<Item, Err> for SharedThrottleObserver<O, S, Item>
where
  O: Observer<Item, Err> + Send + 'static,
  S: SharedScheduler + Send + 'static,
  Item: Clone + Send + 'static,
{
  impl_throttle_observer!(Item, Err, lock.unwrap);
}

impl<O, S, Item, Err> Observer<Item, Err> for LocalThrottleObserver<O, S, Item>
where
  O: Observer<Item, Err> + 'static,
  S: LocalScheduler + 'static,
  Item: Clone + 'static,
{
  impl_throttle_observer!(Item, Err, borrow_mut);
}

#[test]
fn smoke() {
  let x = Arc::new(Mutex::new(vec![]));
  let x_c = x.clone();

  let interval = observable::interval(Duration::from_millis(5));
  let throttle_subscribe = |edge| {
    let x = x.clone();
    interval
      .clone()
      .to_shared()
      .throttle_time(Duration::from_millis(48), edge)
      .to_shared()
      .subscribe(move |v| x.lock().unwrap().push(v))
  };

  // tailing throttle
  let mut sub = throttle_subscribe(ThrottleEdge::Tailing);
  std::thread::sleep(Duration::from_millis(520));
  sub.unsubscribe();
  assert_eq!(
    x_c.lock().unwrap().clone(),
    vec![9, 19, 29, 39, 49, 59, 69, 79, 89, 99]
  );

  // leading throttle
  x_c.lock().unwrap().clear();
  throttle_subscribe(ThrottleEdge::Leading);
  std::thread::sleep(Duration::from_millis(520));
  assert_eq!(
    x_c.lock().unwrap().clone(),
    vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
  );
}

#[test]
fn fork_and_shared() {
  observable::of(0..10)
    .throttle_time(Duration::from_nanos(1), ThrottleEdge::Leading)
    .to_shared()
    .to_shared()
    .subscribe(|_| {});
}
