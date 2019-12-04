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
/// observable::interval!(Duration::from_millis(1))
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

pub struct ThrottleTimeOp<S> {
  source: S,
  duration: Duration,
  edge: ThrottleEdge,
}

impl<S> Fork for ThrottleTimeOp<S>
where
  S: Fork,
{
  type Output = ThrottleTimeOp<S::Output>;
  fn fork(&self) -> Self::Output {
    ThrottleTimeOp {
      source: self.source.fork(),
      edge: self.edge,
      duration: self.duration,
    }
  }
}

impl<S> IntoShared for ThrottleTimeOp<S>
where
  S: IntoShared,
{
  type Shared = ThrottleTimeOp<S::Shared>;
  fn to_shared(self) -> Self::Shared {
    ThrottleTimeOp {
      source: self.source.to_shared(),
      edge: self.edge,
      duration: self.duration,
    }
  }
}

impl<S> ThrottleTime for S {}

impl<Item, Err, O, U, S> RawSubscribable<Item, Err, Subscriber<O, U>>
  for ThrottleTimeOp<S>
where
  S: RawSubscribable<
    Item,
    Err,
    Subscriber<ThrottleTimeObserver<O::Shared, Item>, SharedSubscription>,
  >,
  O: IntoShared,
  U: IntoShared<Shared = SharedSubscription>,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let subscription = subscriber.subscription.to_shared();
    source.raw_subscribe(Subscriber {
      observer: ThrottleTimeObserver(Arc::new(Mutex::new(
        InnerThrottleTimeObserver {
          observer: subscriber.observer.to_shared(),
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

impl<O, Item> IntoShared for ThrottleTimeObserver<O, Item>
where
  O: Send + Sync + 'static,
  Item: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<O, Item, Err> Observer<Item, Err> for ThrottleTimeObserver<O, Item>
where
  O: Observer<Item, Err> + Send + 'static,
  Item: Clone + Send + 'static,
{
  fn next(&mut self, value: &mut Item) {
    let mut inner = self.0.lock().unwrap();
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let c_inner = self.0.clone();
      let subscription = Schedulers::ThreadPool.schedule(
        move |_, _| {
          let mut inner = c_inner.lock().unwrap();
          if let Some(mut v) = inner.trailing_value.take() {
            inner.observer.next(&mut v);
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

  fn error(&mut self, err: &Err) {
    let mut inner = self.0.lock().unwrap();
    inner.observer.error(err)
  }
  fn complete(&mut self) {
    let mut inner = self.0.lock().unwrap();
    if let Some(mut value) = inner.trailing_value.take() {
      inner.observer.next(&mut value);
    }
    inner.observer.complete();
  }
}

#[test]
fn smoke() {
  let x = Arc::new(Mutex::new(vec![]));
  let x_c = x.clone();

  let interval = observable::interval!(Duration::from_millis(5));
  let throttle_subscribe = |edge| {
    let x = x.clone();
    interval
      .fork()
      .to_shared()
      .throttle_time(Duration::from_millis(48), edge)
      .subscribe(move |v| x.lock().unwrap().push(*v))
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
    .fork()
    .to_shared()
    .fork()
    .to_shared()
    .subscribe(|_| {});
}
