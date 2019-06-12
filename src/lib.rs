#![feature(external_doc)]
#![doc(include = "../README.md")]

pub mod error;
pub mod ops;
pub mod subject;
pub use error::{NextWhitoutError, NextWithError, WithErr, WithErrByRef};
pub use subject::Subject;

pub trait Observable<'a>: Sized {
  /// The type of the elements being emitted.
  type Item: Sized;
  //
  type Err;
  // the Subscription subsribe method return.
  type Unsubscribe: Subscription<'a, Err = Self::Err> + 'a;

  fn subscribe_with_err<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item) -> Option<Self::Err>;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    self.subscribe_with_err(move |v| {
      next(v);
      None
    })
  }

  fn broadcast(self) -> Subject<'a, Self::Item, Self::Err>
  where
    Self: 'a,
  {
    Subject::from_stream(self)
  }
}

pub trait Observer {
  type Item;
  type Err;

  fn next(&self, v: Self::Item) -> &Self;

  fn complete(self);

  fn error(self, err: Self::Err);
}

/// Subscription returns from `Observable.subscribe(Subscriber)` to allow unsubscribing.
pub trait Subscription<'a>: Clone {
  type Err;
  /// the action you have designed to accept any error notification from the Observable
  fn on_error<E>(&mut self, err: E) -> &mut Self
  where
    E: Fn(&Self::Err) + 'a;
  /// the action you have designed to accept a completion notification from the Observable
  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a;

  /// This allows deregistering an stream before it has finished receiving all events (i.e. before onCompleted is called).
  fn unsubscribe(self);
}
