use crate::prelude::*;
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
  S: LocalObservable<'static, Item = Item, Err = Err, Unsub = Unsub>,
  Unsub: SubscriptionLike + 'static,
  Item: Clone + 'static,
  SD: LocalScheduler + 'static,
{
  type Unsub = Unsub;

  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  >(
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
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
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
  throttled: Option<SpawnHandle>,
  subscription: Sub,
}

struct SharedThrottleObserver<O, S, Item>(
  Arc<Mutex<ThrottleObserver<O, S, Item, SharedSubscription>>>,
);

struct LocalThrottleObserver<O, S, Item>(
  Rc<RefCell<ThrottleObserver<O, S, Item, LocalSubscription>>>,
);

impl<O, S> Observer for SharedThrottleObserver<O, S, O::Item>
where
  O: Observer + Send + 'static,
  S: SharedScheduler + Send + 'static,
  O::Item: Clone + Send + 'static,
{
  type Item = O::Item;
  type Err = O::Err;
  fn next(&mut self, value: Self::Item) {
    let c_inner = self.0.clone();
    let mut inner = self.0.lock().unwrap();
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let delay = inner.delay;
      let spawn_handle = inner.scheduler.schedule(
        move |_| {
          let mut inner = c_inner.lock().unwrap();
          if let Some(v) = inner.trailing_value.take() {
            inner.observer.next(v);
          }
          if let Some(mut throttled) = inner.throttled.take() {
            throttled.unsubscribe();
          }
        },
        Some(delay),
        (),
      );
      inner.throttled = Some(SpawnHandle::new(spawn_handle.handle.clone()));
      inner.subscription.add(spawn_handle);
      if inner.edge == ThrottleEdge::Leading {
        inner.observer.next(value);
      }
    }
  }

  fn error(&mut self, err: Self::Err) {
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

impl<O, S> Observer for LocalThrottleObserver<O, S, O::Item>
where
  O: Observer + 'static,
  S: LocalScheduler + 'static,
  O::Item: Clone + 'static,
{
  type Item = O::Item;
  type Err = O::Err;
  fn next(&mut self, value: Self::Item) {
    let c_inner = self.0.clone();
    let mut inner = self.0.borrow_mut();
    if inner.edge == ThrottleEdge::Tailing {
      inner.trailing_value = Some(value.clone());
    }

    if inner.throttled.is_none() {
      let delay = inner.delay;
      let spawn_handle = inner.scheduler.schedule(
        move |_| {
          let mut inner = c_inner.borrow_mut();
          if let Some(v) = inner.trailing_value.take() {
            inner.observer.next(v);
          }
          if let Some(mut throttled) = inner.throttled.take() {
            throttled.unsubscribe();
          }
        },
        Some(delay),
        (),
      );
      inner.throttled = Some(SpawnHandle::new(spawn_handle.handle.clone()));
      inner.subscription.add(spawn_handle);
      if inner.edge == ThrottleEdge::Leading {
        inner.observer.next(value);
      }
    }
  }

  fn error(&mut self, err: Self::Err) {
    let mut inner = self.0.borrow_mut();
    inner.observer.error(err)
  }

  fn complete(&mut self) {
    let mut inner = self.0.borrow_mut();
    if let Some(value) = inner.trailing_value.take() {
      inner.observer.next(value);
    }
    inner.observer.complete();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_scheduler::ManualScheduler;

  #[test]
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    let scheduler = ManualScheduler::now();

    let interval =
      observable::interval(Duration::from_millis(5), scheduler.clone());
    let throttle_subscribe = |edge| {
      let x = x.clone();
      interval
        .clone()
        .take(5)
        .throttle_time(Duration::from_millis(11), edge, scheduler.clone())
        .subscribe(move |v| x.borrow_mut().push(v))
    };

    // tailing throttle

    let mut sub = throttle_subscribe(ThrottleEdge::Tailing);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    sub.unsubscribe();
    assert_eq!(&*x_c.borrow(), &[2, 4]);

    // leading throttle
    x_c.borrow_mut().clear();
    throttle_subscribe(ThrottleEdge::Leading);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    assert_eq!(&*x_c.borrow(), &[0, 3]);
  }

  #[test]
  fn fork_and_shared() {
    use futures::executor::ThreadPool;
    let scheduler = ThreadPool::new().unwrap();
    observable::from_iter(0..10)
      .throttle_time(Duration::from_nanos(1), ThrottleEdge::Leading, scheduler)
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }
}
