use crate::prelude::*;
use std::thread;
use std::time::Duration;

pub(crate) fn new_thread_schedule<T: Send + 'static>(
  task: impl FnOnce(SharedSubscription, T) + Send + 'static,
  delay: Option<Duration>,
  state: T,
) -> SharedSubscription {
  let subscription = SharedSubscription::default();
  let mut c_subscription = subscription.clone();
  let task = move || {
    thread::spawn(move || {
      if !subscription.is_closed() {
        task(subscription, state);
      }
    });
  };
  if let Some(delay) = delay {
    c_subscription.add(delay_task(delay, task));
  } else {
    task();
  }
  c_subscription
}
