use std::{
  cell::RefCell,
  fmt::Display,
  task::{Context, Poll},
};

use futures::{
  channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
  ready, Future, FutureExt, StreamExt,
};

use crate::{observable::Observable, observer::Observer};

/// Errors that can prevent an observable future from resolving correctly.
#[derive(Debug, Clone)]
pub enum ObservableError {
  /// The observable had no values.
  Empty,

  /// The observable emitted more than one value.
  MultipleValues,
}

impl Display for ObservableError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ObservableError::Empty => write!(f, "the observable has no values"),
      ObservableError::MultipleValues => {
        write!(f, "the observable emitted more than one value")
      }
    }
  }
}

impl std::error::Error for ObservableError {}

// The value send in the channel
type Message<T, E> = Result<Result<T, E>, ObservableError>;

/// A future that resolves with the value emitted by an observable.
pub struct ObservableFuture<T, E> {
  receiver: RefCell<UnboundedReceiver<Message<T, E>>>,
}

impl<T, E> ObservableFuture<T, E> {
  /// Constructs a new `ObservableFuture<T, E>` that awaits the value emitted by a shared observable.
  pub fn new<S>(observable: S) -> Self
  where
    S: Observable<T, E, ObservableFutureObserver<T, E>>,
  {
    let (sender, receiver) = unbounded::<Message<T, E>>();
    observable
      .actual_subscribe(ObservableFutureObserver { sender, last_value: None });

    ObservableFuture { receiver: RefCell::new(receiver) }
  }
}

impl<T, E> Future for ObservableFuture<T, E> {
  type Output = Result<Result<T, E>, ObservableError>;

  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    let this = self.as_mut();
    let mut receiver = this.receiver.borrow_mut();

    // We poll the receiver stream until the observable completes
    let ret = ready!(receiver.next().poll_unpin(cx));

    match ret {
      Some(msg) => Poll::Ready(msg),
      None => Poll::Pending,
    }
  }
}

pub struct ObservableFutureObserver<T, E> {
  sender: UnboundedSender<Message<T, E>>,
  last_value: Option<Message<T, E>>,
}

impl<T, E> Observer<T, E> for ObservableFutureObserver<T, E> {
  fn next(&mut self, value: T) {
    send_observable_value(self, Ok(value));
  }

  fn error(mut self, err: E) {
    send_observable_value(&mut self, Err(err));
  }

  fn complete(mut self) {
    // When the observable complete we send the last emitted value of the observer and close the channel
    // if not value is emitted an error is sent
    let last_value = self
      .last_value
      .take()
      .unwrap_or(Err(ObservableError::Empty));
    self
      .sender
      .unbounded_send(last_value)
      .expect("failed to send observable last emitted value");
    self.sender.close_channel();
  }

  fn is_finished(&self) -> bool {
    self.sender.is_closed()
  }
}

fn send_observable_value<T, E>(
  observer: &mut ObservableFutureObserver<T, E>,
  value: Result<T, E>,
) {
  match observer.last_value.as_mut() {
    Some(x) => {
      // A future only returns one value
      *x = Err(ObservableError::MultipleValues);
    }
    None => {
      observer.last_value.replace(Ok(value));
    }
  }
}

#[cfg(test)]
mod tests {
  use futures::executor::block_on;

  use crate::{observable::ObservableExt, ops::future::ObservableError};

  #[tokio::test]
  async fn to_future_observable_resolve_value_test() {
    let fut = crate::observable::of(4)
      .map(|x| format!("Number {x}"))
      .to_future();

    let value = fut.await.unwrap().ok().unwrap();
    assert_eq!(format!("Number 4"), value);
  }

  #[test]
  fn to_future_error_empty_observable_test() {
    let fut = ObservableExt::<i32, _>::to_future(crate::observable::empty());
    let value = block_on(fut);
    // let value = fut.await;
    // println!("{value:?}");
    assert!(matches!(value, Err(ObservableError::Empty)));
  }

  #[tokio::test]
  async fn to_future_error_multiple_values_emitted_observable_test() {
    let fut = crate::observable::from_iter([1, 2, 3]).to_future();
    let value = fut.await;

    assert!(matches!(value, Err(ObservableError::MultipleValues)));
  }
}
