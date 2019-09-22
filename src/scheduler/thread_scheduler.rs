use crate::prelude::*;
use std::thread;

pub(crate) fn new_thread_schedule<T: Send + 'static>(
  task: impl FnOnce(SharedSubscription, T) + Send + 'static,
  state: T,
) -> SharedSubscription {
  let subscription = SharedSubscription::default();
  let c_proxy = subscription.clone();
  thread::spawn(move || {
    if !subscription.is_closed() {
      task(subscription, state);
    }
  });
  c_proxy
}
