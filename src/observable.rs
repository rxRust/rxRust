use crate::prelude::*;

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
