use crate::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Emits a value from the source Observable, then ignores subsequent source
/// values for duration milliseconds, then repeats this process.
///
/// #Example
/// ```
/// use rxrust::{ prelude::*, ops::{ ThrottleTime, ThrottleEdge }};
/// use std::time::Duration;
///
/// observable::interval(Duration::from_millis(1))
///   .throttle_time(Duration::from_millis(9), ThrottleEdge::Leading)
///   .subscribe(move |v| println!("{}", v));
/// ```
pub trait ThrottleTime
where
  Self: Sized,
{
  fn throttle_time(
    self,
    duration: Duration,
    edge: ThrottleEdge,
  ) -> ThrottleTimeOp<Self> {
    ThrottleTimeOp {
      source: self,
      duration,
      edge,
    }
  }
}

/// Config to define leading and trailing behavior for throttle
#[derive(PartialEq, Clone, Copy)]
pub enum ThrottleEdge {
  Tailing,
  Leading,
}

#[derive(Clone)]
pub struct ThrottleTimeOp<S> {
  source: S,
  duration: Duration,
  edge: ThrottleEdge,
}

impl<S> ThrottleTime for S {}

impl<Item, Err, S> SharedObservable for ThrottleTimeOp<S>
where
  S: for<'r> Observable<'r, Item = Item, Err = Err>,
  Item: Clone + Send + 'static,
{
  type Item = Item;
  type Err = Err;
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    source.actual_subscribe(Subscriber {
      observer: ThrottleTimeObserver(Arc::new(Mutex::new(
        InnerThrottleTimeObserver {
          observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscription.clone(),
        },
      ))),
      subscription,
    })
  }
}

impl<Item, Err, S> SharedObservable for ThrottleTimeOp<Shared<S>>
where
  S: SharedObservable<Item = Item, Err = Err>,
  Item: Clone + Send + 'static,
{
  type Item = Item;
  type Err = Err;
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    source.0.shared_actual_subscribe(Subscriber {
      observer: ThrottleTimeObserver(Arc::new(Mutex::new(
        InnerThrottleTimeObserver {
          observer,
          edge,
          delay: duration,
          trailing_value: None,
          throttled: None,
          subscription: subscription.clone(),
        },
      ))),
      subscription,
    })
  }
}

struct InnerThrottleTimeObserver<O, Item> {
  observer: O,
  edge: ThrottleEdge,
  delay: Duration,
  trailing_value: Option<Item>,
  throttled: Option<SharedSubscription>,
  subscription: SharedSubscription,
}

pub struct ThrottleTimeObserver<O, Item>(
  Arc<Mutex<InnerThrottleTimeObserver<O, Item>>>,
);

impl<O, Item, Err> Observer<Item, Err> for ThrottleTimeObserver<O, Item>
where
  O: Observer<Item, Err> + Send + 'static,
  Item: Clone + Send + 'static,
{
  fn next(&mut self, value: Item) {
    let mut inner = self.0.lock().unwrap();
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let c_inner = self.0.clone();
      let subscription = Schedulers::ThreadPool.schedule(
        move |_, _| {
          let mut inner = c_inner.lock().unwrap();
          if let Some(v) = inner.trailing_value.take() {
            inner.observer.next(v);
          }
          if let Some(mut throttled) = inner.throttled.take() {
            throttled.unsubscribe();
            inner.subscription.remove(&throttled);
          }
        },
        Some(inner.delay),
        (),
      );
      inner.subscription.add(subscription.clone());
      inner.throttled = Some(subscription);
      if inner.edge == ThrottleEdge::Leading {
        inner.observer.next(value);
      }
    }
  }

  fn error(&mut self, err: Err) {
    let mut inner = self.0.lock().unwrap();
    inner.observer.error(err)
  }

  fn complete(&mut self) {
    let mut inner = self.0.lock().unwrap();
    if let Some(value) = inner.trailing_value.take() {
      inner.observer.next(value);
    }
    inner.observer.complete();
  }
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
  let _guard = throttle_subscribe(ThrottleEdge::Leading);
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
    .clone()
    .to_shared()
    .clone()
    .to_shared()
    .subscribe(|_| {});
}
