use crate::prelude::*;
use futures::prelude::*;
use observable::from_future::DEFAULT_RUNTIME;
use std::sync::mpsc::channel;

pub(crate) fn thread_pool_schedule<
  T: Send + Sync + 'static,
  R: Send + Sync + 'static,
>(
  task: impl FnOnce(Option<T>) -> R + Send + 'static,
  state: Option<T>,
) -> R {
  let (sender, receiver) = channel();
  let f = future::lazy(move |_| {
    sender
      .send(task(state))
      .expect("thread pool scheduler send message failed")
  });
  DEFAULT_RUNTIME.lock().unwrap().spawn_ok(f);
  receiver.recv().unwrap()
}
