use std::sync::{Arc, Mutex};

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait Subscription {
  /// This allows deregistering an stream before it has finished receiving all
  ///  events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);
}

impl Subscription for Box<dyn Subscription + Send + Sync> {
  #[inline(always)]
  fn unsubscribe(&mut self) {
    let s: &mut dyn Subscription = &mut *self;
    s.unsubscribe()
  }
}

struct InnerProxy<T> {
  stopped: bool,
  subscription: Option<T>,
}

pub struct SubscriptionProxy<T>(Arc<Mutex<InnerProxy<T>>>);

impl<T> Clone for SubscriptionProxy<T> {
  fn clone(&self) -> Self { SubscriptionProxy(self.0.clone()) }
}

impl<T> Subscription for SubscriptionProxy<T>
where
  T: Subscription,
{
  fn unsubscribe(&mut self) { Self::unsubscribe(self) }
}

impl<T> Default for SubscriptionProxy<T>
where
  T: Subscription,
{
  fn default() -> Self { Self::new() }
}

impl<T> SubscriptionProxy<T>
where
  T: Subscription,
{
  pub fn new() -> Self {
    SubscriptionProxy(Arc::new(Mutex::new(InnerProxy {
      stopped: false,
      subscription: None,
    })))
  }

  pub fn proxy(&self, mut target: T) {
    let mut inner = self.0.lock().unwrap();
    if inner.stopped {
      target.unsubscribe();
    } else {
      inner.subscription.replace(target);
    }
  }

  pub fn unsubscribe(&self) {
    let mut inner = self.0.lock().unwrap();
    if !inner.stopped {
      if let Some(ref mut s) = inner.subscription {
        s.unsubscribe();
      }
      inner.stopped = true;
    }
  }
}
