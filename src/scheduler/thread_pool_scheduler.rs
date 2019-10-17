use crate::prelude::*;
use std::time::Duration;

pub(crate) fn thread_pool_schedule<T: Send + Sync + 'static>(
  task: impl FnOnce(SharedSubscription, T) + Send + 'static,
  delay: Option<Duration>,
  state: T,
) -> SharedSubscription {
  let mut subscription = SharedSubscription::default();
  let c_subscription = subscription.clone();
  let delay = delay.unwrap_or_default();

  let s = delay_task(delay, move || task(c_subscription, state));
  subscription.add(s);
  subscription
}
