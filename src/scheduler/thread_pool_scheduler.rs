use crate::prelude::*;
use futures::prelude::*;
use observable::from_future::DEFAULT_RUNTIME;

pub(crate) fn thread_pool_schedule<T: Send + Sync + 'static>(
  task: impl FnOnce(Option<T>) + Send + 'static,
  state: Option<T>,
) {
  let f = future::lazy(move |_| task(state));
  DEFAULT_RUNTIME.lock().unwrap().spawn_ok(f);
}
