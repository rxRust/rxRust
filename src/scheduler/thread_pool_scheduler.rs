use crate::observable::{from_future::DEFAULT_RUNTIME, interval::SpawnHandle};
use crate::prelude::*;
use futures::prelude::*;
use futures::task::SpawnExt;

pub(crate) fn thread_pool_schedule<T: Send + Sync + 'static>(
  task: impl FnOnce(SubscriptionProxy, Option<T>) + Send + 'static,
  state: Option<T>,
) -> SubscriptionProxy {
  let proxy = SubscriptionProxy::new();
  let c_proxy = proxy.clone();
  let f = future::lazy(move |_| task(c_proxy, state));
  let handle = DEFAULT_RUNTIME
    .lock()
    .unwrap()
    .spawn_with_handle(f)
    .expect("spawn task to thread pool failed.");

  proxy.proxy(Box::new(SpawnHandle(Some(handle))));
  proxy
}
