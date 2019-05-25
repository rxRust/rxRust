#![cfg_attr(feature = "nightly", feature(external_doc))]
#![cfg_attr(feature = "nightly", doc(include = "../README.md"))]

pub mod ops;
pub mod subject;

pub use subject::Subject;

pub trait Observable<'a>: Sized {
  /// The type of the elements being emitted.
  type Item: Sized;
  //
  type Err;
  // the Subscription subsribe method return.
  type Unsubscribe: Subscription<'a, Err = Self::Err>;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item);

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

pub trait Subscription<'a> {
  type Err;
  fn on_error<E>(&mut self, err: E) -> &mut Self
  where
    E: Fn(&Self::Err) + 'a;
  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a;
  fn unsubscribe(self);
}
