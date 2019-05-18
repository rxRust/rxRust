
pub mod ops;
pub mod subject;

pub use subject::Subject;

pub trait Observable<'a>: Sized {
  /// The type of the elements being emitted.
  type Item: Sized;
  // the Subscription subsribe method return.
  type Unsubscribe: Subscription;

  fn subscribe<O>(self, observer: O) -> Self::Unsubscribe
  where
    O: 'a + FnMut(Self::Item);


  fn broadcast(self) -> Subject<'a, Self::Item>
  where
    Self: 'a,
  {
    Subject::from_stream(self)
  }
}

pub trait Observer {
  type Item;

  fn next(&self, v: Self::Item) -> &Self;
}


pub trait Subscription {
  fn unsubscribe(self);
}