use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
use std::time::Duration;

#[derive(Clone)]
pub struct DelayOp<S, SD> {
  pub(crate) source: S,
  pub(crate) delay: Duration,
  pub(crate) scheduler: SD,
}

impl<S: Observable, SD> Observable for DelayOp<S, SD> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, SD> DelayOp<S, SD>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident){
    let subscription = $ctx::RcMultiSubscription::default();
    let c_subscription = subscription.clone();
    let handle = $self.scheduler.schedule(
      move |_| {
        c_subscription.add($self.source.actual_subscribe($observer));
      },
      Some($self.delay),
      (),
    );
    subscription.add(handle);
    subscription
  }
  where
    @ctx::local_only('o: 'static,)
    S: @ctx::Observable @ctx::shared_only(+ Send) + 'static,
    S::Unsub: 'static,
    SD: @ctx::Scheduler
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::LocalPool;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::{Arc, Mutex};
  use std::time::Instant;
  use std::{cell::RefCell, rc::Rc};

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared_smoke() {
    let value = Arc::new(Mutex::new(0));
    let c_value = value.clone();
    let pool = ThreadPool::new().unwrap();
    let stamp = Instant::now();
    observable::of(1)
      .delay(Duration::from_millis(50), pool)
      .into_shared()
      .subscribe_blocking(move |v| {
        *value.lock().unwrap() = v;
      });
    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*c_value.lock().unwrap(), 1);
  }

  #[test]
  fn local_smoke() {
    let value = Rc::new(RefCell::new(0));
    let c_value = value.clone();
    let mut pool = LocalPool::new();
    observable::of(1)
      .delay(Duration::from_millis(50), pool.spawner())
      .subscribe(move |v| {
        *c_value.borrow_mut() = v;
      });
    assert_eq!(*value.borrow(), 0);
    let stamp = Instant::now();
    pool.run();
    assert!(stamp.elapsed() > Duration::from_millis(50));
    assert_eq!(*value.borrow(), 1);
  }
}
