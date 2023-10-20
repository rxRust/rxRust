use std::{
  cell::RefCell,
  task::{Context, Poll},
};

use futures::{
  channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
  ready, FutureExt, Stream, StreamExt,
};

use crate::{observable::Observable, observer::Observer};

enum Message<T, E> {
  Item(Result<T, E>),
  Complete,
}

/// A stream that returns the values emitted by an observable.
pub struct ObservableStream<T, E> {
  receiver: RefCell<UnboundedReceiver<Message<T, E>>>,
}

impl<T, E> ObservableStream<T, E> {
  /// Constructs a new `ObservableStream<T, E>` that emits the values from the observable.
  pub fn new<O>(observable: O) -> Self
  where
    O: Observable<T, E, ObservableStreamObserver<T, E>>,
  {
    let (sender, receiver) = unbounded::<Message<T, E>>();
    observable.actual_subscribe(ObservableStreamObserver { sender });

    ObservableStream { receiver: RefCell::new(receiver) }
  }
}

impl<T, E> Stream for ObservableStream<T, E> {
  type Item = Result<T, E>;

  fn poll_next(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    let this = self.as_mut();
    let mut receiver = this.receiver.borrow_mut();

    // We poll the UnboundedReceiver stream until the observable completes
    let ret = ready!(receiver.next().poll_unpin(cx));
    match ret {
      Some(msg) => match msg {
        Message::Item(x) => Poll::Ready(Some(x)),
        Message::Complete => {
          receiver.close();
          Poll::Ready(None)
        }
      },
      None => Poll::Pending,
    }
  }
}

pub struct ObservableStreamObserver<T, E> {
  sender: UnboundedSender<Message<T, E>>,
}

impl<T, E> Observer<T, E> for ObservableStreamObserver<T, E> {
  fn next(&mut self, value: T) {
    self
      .sender
      .unbounded_send(Message::Item(Ok(value)))
      .expect("failed to send value to stream");
  }

  fn error(self, err: E) {
    self
      .sender
      .unbounded_send(Message::Item(Err(err)))
      .expect("failed to send error to stream");
  }

  fn complete(self) {
    self
      .sender
      .unbounded_send(Message::Complete)
      .expect("failed to send a complete message");
  }

  fn is_finished(&self) -> bool {
    self.sender.is_closed()
  }
}

#[cfg(test)]
mod tests {
  use crate::observable::ObservableExt;
  use futures::StreamExt;

  #[tokio::test]
  async fn to_stream_receive_all_values_test() {
    let mut stream = crate::observable::from_iter([1, 2, 3]).to_stream();

    let mut values = vec![];
    while let Some(Ok(x)) = stream.next().await {
      values.push(x);
    }

    assert_eq!(vec![1, 2, 3], values);
  }

  #[tokio::test]
  async fn to_stream_empty_observable_test() {
    let mut stream = crate::observable::empty().to_stream();

    let mut values: Vec<i32> = vec![];
    while let Some(Ok(x)) = stream.next().await {
      values.push(x);
    }

    assert!(values.is_empty());
  }
}
