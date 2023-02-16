use futures::{ready, Future, Stream};
use pin_project_lite::pin_project;
use std::{
  pin::Pin,
  task::{Context, Poll},
};

use crate::{
  observer::Observer,
  scheduler::{NormalReturn, Scheduler, TaskHandle},
};

use super::{Observable, ObservableExt};

/// Returns an `Observable` that emits all the items returned from the source `Stream<Result<Item, Err>`
/// until it completes or returns an error.
///
/// This is similar to [`rxrust::from_stream`] but errors should be handled.
pub fn from_stream_result<Item, Err, S, SD>(
  stream: S,
  scheduler: SD,
) -> TryStreamObservable<S, SD>
where
  S: Stream<Item = Result<Item, Err>>,
{
  TryStreamObservable { stream, scheduler }
}

#[derive(Clone)]
pub struct TryStreamObservable<S, SD> {
  stream: S,
  scheduler: SD,
}

impl<Item, Err, O, S, SD> Observable<Item, Err, O>
  for TryStreamObservable<S, SD>
where
  S: Stream<Item = Result<Item, Err>>,
  O: Observer<Item, Err>,
  SD: Scheduler<TryStreamObserverFuture<S, O>>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { stream, scheduler } = self;
    let task = TryStreamObserverFuture { stream, observer: Some(observer) };
    scheduler.schedule(task, None)
  }
}

impl<Item, Err, S, SD> ObservableExt<Item, Err> for TryStreamObservable<S, SD> where
  S: Stream<Item = Result<Item, Err>>
{
}

pin_project! {
    struct TryStreamObserverFuture<S, O> {
        #[pin]
        stream: S,
        observer: Option<O>,
    }
}

impl<Item, Err, S, O> Future for TryStreamObserverFuture<S, O>
where
  S: Stream<Item = Result<Item, Err>>,
  O: Observer<Item, Err>,
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
        Some(Ok(value)) => {
          // Send the item to the observer
          this
            .observer
            .as_mut()
            .expect("future polled before done")
            .next(value);
        }
        Some(Err(err)) => {
          let observer =
            this.observer.take().expect("future polled before done");
          observer.error(err);

          // exit on error
          break Poll::Ready(NormalReturn::new(()));
        }
        None => {
          let observer =
            this.observer.take().expect("future polled before done");
          observer.complete();

          // exit on complete
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
  fn from_error_stream() {
    let error_stream = futures::stream::once(async {
      Err::<(), String>("uh oh stinky".to_owned())
    });

    let mut scheduler = LocalPool::new();
    let error_count = MutRc::own(0);

    {
      let error_count = error_count.clone();

      let observable = from_stream_result(error_stream, scheduler.spawner());
      let _s = observable
        .on_error(move |_| {
          *error_count.rc_deref_mut() += 1;
        })
        .subscribe(|_| {});

      scheduler.run();
    }

    assert_eq!(*error_count.rc_deref(), 1);
  }

  #[test]
  fn from_stream_result_test() {
    let stream = futures::stream::try_unfold(1, |state| async move {
      if state == 4 {
        Err("invalid value".to_owned())
      } else {
        Ok(Some((state, state + 1)))
      }
    });

    let mut scheduler = LocalPool::new();
    let observable = from_stream_result(stream, scheduler.spawner());

    let values = MutRc::own(vec![]);
    let error_count = MutRc::own(0);
    {
      let values = values.clone();
      let error_count = error_count.clone();
      let _s = observable
        .on_error(move |_| {
          *error_count.rc_deref_mut() += 1;
        })
        .subscribe(move |x| {
          values.rc_deref_mut().push(x);
        });
    }

    scheduler.run();

    let cur = values.rc_deref().clone();
    assert_eq!(cur, vec![1, 2, 3]);
    assert_eq!(*error_count.rc_deref(), 1);
  }

  #[tokio::test]
  #[cfg(all(test, not(target_arch = "wasm32"), feature = "tokio-scheduler"))]
  async fn stream_channel_test() {
    use crate::rc::MutArc;
    use futures::{channel::mpsc::channel, SinkExt};
    use std::time::Duration;

    struct UnexpectedError;
    let (mut sender, receiver) = channel::<Result<i32, UnexpectedError>>(3);
    let values = MutArc::own(vec![]);
    let error_count = MutArc::own(0);

    let scheduler = FuturesThreadPoolScheduler::new().unwrap();
    let observable = from_stream_result(receiver, scheduler.clone());

    {
      let values = values.clone();
      let error_count = error_count.clone();

      let _s = observable
        .on_error(move |_| {
          *error_count.rc_deref_mut() += 1;
        })
        .subscribe(move |x| {
          values.rc_deref_mut().push(x);
        });
    }

    sender.send(Ok(1)).await.unwrap();
    sender.send(Ok(2)).await.unwrap();
    sender.send(Err(UnexpectedError)).await.unwrap();
    sender.send(Ok(3)).await.unwrap();

    // Waits for spawn all futures
    tokio::time::sleep(Duration::from_millis(200)).await;

    let cur = values.rc_deref().clone();
    assert_eq!(cur, vec![1, 2]);
    assert_eq!(*error_count.rc_deref(), 1);
  }
}
