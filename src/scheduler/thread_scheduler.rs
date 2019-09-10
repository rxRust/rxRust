use crate::prelude::*;
use std::thread;

pub(crate) fn new_thread_schedule<T: Send + 'static>(
  task: impl FnOnce(SubscriptionProxy, Option<T>) + Send + 'static,
  state: Option<T>,
) -> SubscriptionProxy {
  let proxy = SubscriptionProxy::new();
  let c_proxy = proxy.clone();
  thread::spawn(move || {
    if !proxy.is_stopped() {
      task(proxy, state);
    }
  });
  c_proxy
}
