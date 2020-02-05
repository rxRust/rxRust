use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverErr<N, E> {
  next: N,
  error: E,
}

impl<N, E> ObserverComplete for ObserverErr<N, E> {
  #[inline(always)]
  fn complete(&mut self) {}
}

impl<N, E, Item> ObserverNext<Item> for ObserverErr<N, E>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, err: Item) { (self.next)(err); }
}

impl<N, E, Err> ObserverError<Err> for ObserverErr<N, E>
where
  E: FnMut(Err),
{
  #[inline(always)]
  fn error(&mut self, err: Err) { (self.error)(err); }
}

impl<N, E> ObserverErr<N, E> {
  #[inline(always)]
  pub fn new(next: N, error: E) -> Self { ObserverErr { next, error } }
}

impl<N, E> IntoShared for ObserverErr<N, E>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribeErr<N, E> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// completion.
  ///
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub;
}

impl<S, N, E> SubscribeErr<N, E> for S
where
  S: Observable<ObserverErr<N, E>, LocalSubscription>,
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub
  where
    Self: Sized,
  {
    self.actual_subscribe(Subscriber::local(ObserverErr { next, error }))
  }
}
