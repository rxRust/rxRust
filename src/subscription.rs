use std::sync::{Arc, Mutex};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait Subscription {
  /// This allows deregistering an stream before it has finished receiving all
  /// events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);

  fn is_stopped(&self) -> bool;
}

struct InnerProxy {
  stopped: bool,
  subscription: Vec<Box<dyn Subscription + Send + Sync>>,
}

pub struct SubscriptionProxy(Arc<Mutex<InnerProxy>>);

impl Clone for SubscriptionProxy {
  fn clone(&self) -> Self { SubscriptionProxy(self.0.clone()) }
}

impl Subscription for SubscriptionProxy {
  fn unsubscribe(&mut self) { Self::unsubscribe(self) }

  fn is_stopped(&self) -> bool { self.0.lock().unwrap().stopped }
}

impl Default for SubscriptionProxy {
  fn default() -> Self { Self::new() }
}

impl SubscriptionProxy {
  pub fn new() -> Self {
    SubscriptionProxy(Arc::new(Mutex::new(InnerProxy {
      stopped: false,
      subscription: vec![],
    })))
  }

  pub fn proxy<S>(&self, mut target: S)
  where
    S: Subscription + Send + Sync + 'static,
  {
    let mut inner = self.0.lock().unwrap();
    if inner.stopped {
      target.unsubscribe();
    } else {
      inner.subscription.push(Box::new(target));
    }
  }

  pub fn unsubscribe(&self) {
    let mut inner = self.0.lock().unwrap();
    if !inner.stopped {
      inner.subscription.iter_mut().for_each(|s| s.unsubscribe());
      inner.stopped = true;
    }
  }
}
