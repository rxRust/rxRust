use futures::{ready, Future, Stream};
use pin_project_lite::pin_project;
use std::{
  convert::Infallible,
  pin::Pin,
  task::{Context, Poll},
};

use crate::{
  observer::Observer,
  scheduler::{NormalReturn, Scheduler, TaskHandle},
};

use super::{Observable, ObservableExt};

/// Returns an `Observable` that emits all the items returned from the source `Stream`.
///
/// ```rust
/// use rxrust::prelude::*;
/// use futures::{executor::LocalPool};
///
/// let stream = futures::stream::unfold(1, |state| async move {
///     if state < 4 {
///         Some((state, state + 1))
///     } else {
///         None
///     }
/// });
///
/// let mut local_scheduler = LocalPool::new();
/// let observable = from_stream(stream, local_scheduler.spawner());
/// observable.subscribe(|x| {
///     println!("{x}");
/// });
///
/// local_scheduler.run();
///
/// // prints:
/// // 1
/// // 2
/// // 3
/// ```
///
/// # Remarks
/// If you want convert a `Stream` that can fail use [`rxrust::from_stream_result`] instead.
pub fn from_stream<S, SD>(stream: S, scheduler: SD) -> StreamObservable<S, SD>
where
  S: Stream,
{
  StreamObservable { stream, scheduler }
}

#[derive(Clone)]
pub struct StreamObservable<S, SD> {
  stream: S,
  scheduler: SD,
}

impl<O, S, SD> Observable<S::Item, Infallible, O> for StreamObservable<S, SD>
where
  S: Stream,
  O: Observer<S::Item, Infallible>,
  SD: Scheduler<StreamObserverFuture<S, O>>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { stream, scheduler } = self;
    let task = StreamObserverFuture { stream, observer: Some(observer) };
    scheduler.schedule(task, None)
  }
}

impl<S, SD> ObservableExt<S::Item, Infallible> for StreamObservable<S, SD> where
  S: Stream
{
}

pin_project! {
    struct StreamObserverFuture<S, O> {
        #[pin]
        stream: S,
        observer: Option<O>,
    }
}

impl<S, O> Future for StreamObserverFuture<S, O>
where
  S: Stream,
  O: Observer<S::Item, Infallible>,
{
  type Output = NormalReturn<()>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    loop {
      // Poll the stream until exhausted
      let this = self.as_mut().project();
      let next = ready!(this.stream.poll_next(cx));

      match next {
        Some(value) => {
          // Send the item to the observer
          this
            .observer
            .as_mut()
            .expect("future polled before done")
            .next(value);
        }
        None => {
          let observer =
            this.observer.take().expect("future polled before done");
          observer.complete();
          break Poll::Ready(NormalReturn::new(()));
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    prelude::*,
    rc::{MutRc, RcDeref, RcDerefMut},
  };
  use futures::executor::LocalPool;

  #[test]
  fn from_empty_stream() {
    let empty = futures::stream::empty::<String>();
    let mut scheduler = LocalPool::new();

    let count = MutRc::own(0);
    {
      let count = count.clone();
      from_stream(empty, scheduler.spawner()).subscribe(move |_| {
        *count.rc_deref_mut() += 1;
      });
    }

    scheduler.run();
    assert_eq!(*count.rc_deref(), 0);
  }

  #[test]
  fn from_stream_test() {
    let stream = futures::stream::unfold(1, |state| async move {
      if state < 4 {
        Some((state, state + 1))
      } else {
        None
      }
    });

    let mut scheduler = LocalPool::new();
    let observable = from_stream(stream, scheduler.spawner());

    let values = MutRc::own(vec![]);
    {
      let values = values.clone();
      observable.subscribe(move |x| {
        values.rc_deref_mut().push(x);
      });
    }

    scheduler.run();

    let cur = values.rc_deref().clone();
    assert_eq!(cur, vec![1, 2, 3])
  }

  #[tokio::test]
  #[cfg(all(test, not(target_arch = "wasm32"), feature = "tokio-scheduler"))]
  async fn stream_channel_test() {
    use crate::rc::MutArc;
    use futures::{channel::mpsc::channel, SinkExt};
    use std::time::Duration;

    let (mut sender, receiver) = channel(3);

    let scheduler = FuturesThreadPoolScheduler::new().unwrap();
    let observable = from_stream(receiver, scheduler.clone());

    let values = MutArc::own(vec![]);
    {
      let values = values.clone();
      observable.subscribe(move |x| {
        values.rc_deref_mut().push(x);
      });
    }

    sender.send(1).await.unwrap();
    sender.send(2).await.unwrap();
    sender.send(3).await.unwrap();

    // Waits for spawn all futures
    tokio::time::sleep(Duration::from_millis(200)).await;

    let cur = values.rc_deref().clone();
    assert_eq!(cur, vec![1, 2, 3]);
  }
}
