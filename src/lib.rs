#![feature(external_doc)]
#![doc(include = "../README.md")]

pub mod ops;
pub mod subject;

pub use subject::Subject;

pub trait Observable<'a>: Sized {
  /// The type of the elements being emitted.
  type Item: Sized;
  /// The type of error may occur.
  type Err;
  // the Subscription subsribe method return.
  type Unsubscribe: Subscription;

  fn subscribe<N, EC>(self, next: N, err_or_complete: EC) -> Self::Unsubscribe
  where
    N: 'a + FnMut(Self::Item),
    EC: 'a + FnMut(&ErrComplete<Self::Err>);

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

  fn err(self, err: Self::Err);
}

pub trait Subscription {
  fn unsubscribe(self);
}

pub enum ErrComplete<E> {
  Complete,
  Err(E),
}