use futures::Future;

use crate::{
  observable::{ObservableImpl, Observable},
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

pub fn complete_status<Item, Err, S: Observable<Item, Err>>(
  source: S,
) -> (StatusOp<S>, Arc<CompleteStatus>) {
  let status = Arc::new(CompleteStatus::default());
  (StatusOp { source, status: status.clone() }, status)
}

impl<S, Item, Err, O> ObservableImpl<Item, Err, O> for StatusOp<S>
where
  O: Observer<Item, Err>,
  S: ObservableImpl<Item, Err, StatusObserver<O>>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, status } = self;
    source.actual_subscribe(StatusObserver { observer, status })
  }
}

impl<S, Item, Err> Observable<Item, Err> for StatusOp<S> where
  S: Observable<Item, Err>
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

  /// Wait until the observable completes or an error occurs.
  pub async fn wait_completed(&self) -> NormalReturn<()> {
    let flag = &self.flag;
    let waker = &self.waker;

    struct StatusFuture<'a> {
      flag: &'a AtomicI8,
      waker: &'a futures::task::AtomicWaker,
    }

    impl<'a> Future for StatusFuture<'a> {
      type Output = NormalReturn<()>;

      fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
      ) -> Poll<Self::Output> {
        if self.flag.load(Ordering::Relaxed) != 0 {
          Poll::Ready(NormalReturn::new(()))
        } else {
          self.waker.register(cx.waker());
          Poll::Pending
        }
      }
    }

    StatusFuture { flag, waker }.await
  }
}

