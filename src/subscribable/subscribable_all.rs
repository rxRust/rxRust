use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeAll<N, E, C> {
  next: N,
  error: E,
  complete: C,
}

impl<N, E, C> SubscribeAll<N, E, C> {
  pub fn new(next: N, error: E, complete: C) -> Self {
    SubscribeAll {
      next,
      error,
      complete,
    }
  }
}

impl<N, E, C> IntoShared for SubscribeAll<N, E, C>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<Item, Err, N, E, C> Subscribe<Item, Err> for SubscribeAll<N, E, C>
where
  N: FnMut(&Item),
  E: FnMut(&Err),
  C: FnMut(),
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { (self.next)(value); }
  #[inline(always)]
  fn on_error(&mut self, err: &Err) { (self.error)(err); }
  #[inline(always)]
  fn on_complete(&mut self) { (self.complete)(); }
}

pub trait SubscribableAll<Item, Err, N, E, C> {
  /// a type implemented [`Subscription`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  ///
  fn subscribe_all(self, next: N, error: E, complete: C) -> Self::Unsub;
}

impl<S, Item, Err, N, E, C> SubscribableAll<Item, Err, N, E, C> for S
where
  S: RawSubscribable<Item, Err, SubscribeAll<N, E, C>>,
  N: FnMut(&Item),
  E: FnMut(&Err),
  C: FnMut(),
{
  type Unsub = S::Unsub;
  fn subscribe_all(self, next: N, error: E, complete: C) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(SubscribeAll {
      next,
      error,
      complete,
    })
  }
}
