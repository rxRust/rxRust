use crate::prelude::*;

pub(crate) fn sync_schedule<T: Send + Sync + 'static>(
  task: impl FnOnce(SubscriptionProxy, Option<T>) + Send + 'static,
  state: Option<T>,
) -> SubscriptionProxy {
  let proxy = SubscriptionProxy::new();
  let c_proxy = proxy.clone();
  task(c_proxy, state);
  proxy
}
