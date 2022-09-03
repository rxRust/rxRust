use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
use std::time::Duration;

/// Config to define leading and trailing behavior for throttle
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ThrottleEdge {
  Tailing,
  Leading,
}

#[derive(Clone)]
pub struct ThrottleOp<S, SD, F> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
  pub(crate) duration_selector: F,
  pub(crate) edge: ThrottleEdge,
}

impl<S, SD, F> Observable for ThrottleOp<S, SD, F>
where
  S: Observable,
  F: Fn(&S::Item) -> Duration,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, SD, F> ThrottleOp<S, SD, F>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let Self {
      source,
      scheduler,
      duration_selector,
      edge,
    } = $self;

    source.actual_subscribe($ctx::Rc::own(
      ThrottleObserver {
        observer: $observer,
        edge,
        duration_selector,
        trailing_value: None,
        throttled: None,
        subscription: ProxySubscription::default(),
        scheduler,
      },
    ))
  }
  where
    @ctx::local_only('o: 'static,)
    S: @ctx::Observable,
    SD: @ctx::Scheduler @ctx::shared_only(+ Send) + 'static,
    S::Item: Clone @ctx::shared_only(+ Send) + 'static,
    F: Fn(&S::Item) -> Duration @ctx::shared_only(+ Send) + 'static,
}

struct ThrottleObserver<O: Observer, SD, F> {
  scheduler: SD,
  observer: O,
  edge: ThrottleEdge,
  duration_selector: F,
  trailing_value: Option<O::Item>,
  throttled: Option<SpawnHandle>,
  subscription: ProxySubscription<SpawnHandle>,
}

macro_rules! impl_observer {
  ($rc: ident, $sd: ident $(,$send: ident)?) => {
    impl<O, SD, F> Observer for $rc<ThrottleObserver<O, SD, F>>
    where
      O: Observer $(+ $send)? + 'static,
      SD: $sd $(+ $send)? + 'static,
      O::Item: Clone $(+ $send)? + 'static,
      F: Fn(&O::Item) -> Duration $(+ $send)? + 'static,
    {
      type Item = O::Item;
      type Err = O::Err;
      fn next(&mut self, value: Self::Item) {
        let c_inner = self.clone();
        let mut inner = self.rc_deref_mut();
        if inner.edge == ThrottleEdge::Tailing {
          inner.trailing_value = Some(value.clone());
        }

        if inner.throttled.is_none() {
          let delay = match inner.edge {
            ThrottleEdge::Leading => (inner.duration_selector)(&value),
            ThrottleEdge::Tailing => {
              if let Some(trailing_value) = inner.trailing_value.take() {
                (inner.duration_selector)(&trailing_value)
              } else {
                Duration::from_secs(0)
              }
            }
          };
          let spawn_handle = inner.scheduler.schedule(
            move |_| {
              let mut inner = c_inner.rc_deref_mut();
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
          inner.subscription.proxy(spawn_handle);
          if inner.edge == ThrottleEdge::Leading {
            inner.observer.next(value);
          }
        }
      }

      fn error(&mut self, err: Self::Err) {
        let mut inner = self.rc_deref_mut();
        inner.observer.error(err)
      }

      fn complete(&mut self) {
        let mut inner = self.rc_deref_mut();
        if let Some(value) = inner.trailing_value.take() {
          inner.observer.next(value);
        }
        inner.observer.complete();
      }
    }
  };
}

impl_observer!(MutRc, LocalScheduler);
#[cfg(not(all(target_arch = "wasm32")))]
impl_observer!(MutArc, SharedScheduler, Send);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_scheduler::ManualScheduler;

  #[test]
  fn smoke() {
    let x = MutRc::own(vec![]);
    let x_c = x.clone();
    let scheduler = ManualScheduler::now();

    let interval =
      observable::interval(Duration::from_millis(5), scheduler.clone());
    let throttle_subscribe = |edge| {
      let x = x.clone();
      interval
        .clone()
        .take(5)
        .throttle(
          |val| -> Duration {
            if val % 2 == 0 {
              Duration::from_millis(7)
            } else {
              Duration::from_millis(5)
            }
          },
          edge,
          scheduler.clone(),
        )
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };

    // tailing throttle
    let mut sub = throttle_subscribe(ThrottleEdge::Tailing);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    sub.unsubscribe();
    assert_eq!(&*x_c.rc_deref(), &[1, 3]);

    // leading throttle
    x_c.rc_deref_mut().clear();
    throttle_subscribe(ThrottleEdge::Leading);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    assert_eq!(&*x_c.rc_deref(), &[0, 2, 4]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn fork_and_shared() {
    use futures::executor::ThreadPool;
    let scheduler = ThreadPool::new().unwrap();
    observable::from_iter(0..10)
      .throttle(
        |val| -> Duration {
          if val % 2 == 0 {
            Duration::from_millis(7)
          } else {
            Duration::from_millis(5)
          }
        },
        ThrottleEdge::Leading,
        scheduler,
      )
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn smoke_for_throttle_time() {
    let x = MutRc::own(vec![]);
    let x_c = x.clone();
    let scheduler = ManualScheduler::now();

    let interval =
      observable::interval(Duration::from_millis(5), scheduler.clone());
    let throttle_time_subscribe = |edge| {
      let x = x.clone();
      interval
        .clone()
        .take(5)
        .throttle_time(Duration::from_millis(11), edge, scheduler.clone())
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };

    // tailing throttle
    let mut sub = throttle_time_subscribe(ThrottleEdge::Tailing);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    sub.unsubscribe();
    assert_eq!(&*x_c.rc_deref(), &[2, 4]);

    // leading throttle
    x_c.rc_deref_mut().clear();
    throttle_time_subscribe(ThrottleEdge::Leading);
    scheduler.advance_and_run(Duration::from_millis(1), 25);
    assert_eq!(&*x_c.rc_deref(), &[0, 3]);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn fork_and_shared_for_throttle_time() {
    use futures::executor::ThreadPool;
    let scheduler = ThreadPool::new().unwrap();
    observable::from_iter(0..10)
      .throttle_time(Duration::from_nanos(1), ThrottleEdge::Leading, scheduler)
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }
}
