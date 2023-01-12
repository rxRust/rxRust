use futures::{executor::block_on, Future};

use crate::{
  observable::{Observable, ObservableExt},
  observer::Observer,
  scheduler::NormalReturn,
};
use std::{
  sync::{
    atomic::{AtomicI8, Ordering},
    Arc,
  },
  task::Poll,
};

#[derive(Default)]
pub struct CompleteStatus {
  flag: AtomicI8,
  waker: futures::task::AtomicWaker,
}
pub struct StatusOp<S> {
  source: S,
  status: Arc<CompleteStatus>,
}

pub fn complete_status<Item, Err, S: ObservableExt<Item, Err>>(
  source: S,
) -> (StatusOp<S>, Arc<CompleteStatus>) {
  let status = Arc::new(CompleteStatus::default());
  (StatusOp { source, status: status.clone() }, status)
}

impl<S, Item, Err, O> Observable<Item, Err, O> for StatusOp<S>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, StatusObserver<O>>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, status } = self;
    source.actual_subscribe(StatusObserver { observer, status })
  }
}

impl<S, Item, Err> ObservableExt<Item, Err> for StatusOp<S> where
  S: ObservableExt<Item, Err>
{
}

pub struct StatusObserver<O> {
  observer: O,
  status: Arc<CompleteStatus>,
}

impl<Item, Err, O> Observer<Item, Err> for StatusObserver<O>
where
  O: Observer<Item, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next(value)
  }

  fn error(self, err: Err) {
    self.observer.error(err);
    self.status.flag.store(-1, Ordering::Relaxed);
    self.status.waker.wake();
  }

  fn complete(self) {
    self.observer.complete();
    self.status.flag.store(1, Ordering::Relaxed);
    self.status.waker.wake();
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

impl CompleteStatus {
  /// return true if the observable completed or emit an error.
  pub fn is_closed(&self) -> bool {
    self.flag.load(Ordering::Relaxed) != 0
  }

  /// return true if the observable completed.
  pub fn is_completed(&self) -> bool {
    self.flag.load(Ordering::Relaxed) > 0
  }

  /// return true if the observable emit an error.
  pub fn error_occur(&self) -> bool {
    self.flag.load(Ordering::Relaxed) < 0
  }

  /// Wait until the observable complete or an error occur.
  pub fn wait_for_end(this: Arc<Self>) {
    block_on(StatusFuture(this));
  }
}

struct StatusFuture(Arc<CompleteStatus>);
impl Future for StatusFuture {
  type Output = NormalReturn<()>;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Self::Output> {
    if self.0.is_closed() {
      Poll::Ready(NormalReturn::new(()))
    } else {
      self.0.waker.register(cx.waker());
      Poll::Pending
    }
  }
}
