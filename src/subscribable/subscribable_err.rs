use crate::prelude::*;

#[derive(Clone)]
pub struct SubscribeErr<N, E> {
  next: N,
  error: E,
}

impl<Item, Err, N, E> Observer<Item, Err> for SubscribeErr<N, E>
where
  N: FnMut(Item),
  E: FnMut(Err),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
  #[inline(always)]
  fn error(&mut self, err: Err) { (self.error)(err); }
  #[inline(always)]
  fn complete(&mut self) {}
}

impl<N, E> SubscribeErr<N, E> {
  #[inline(always)]
  pub fn new(next: N, error: E) -> Self { SubscribeErr { next, error } }
}

impl<N, E> IntoShared for SubscribeErr<N, E>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribableErr<N, E> {
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

impl<S, N, E> SubscribableErr<N, E> for S
where
  S: RawSubscribable<Subscriber<SubscribeErr<N, E>, LocalSubscription>>,
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> Self::Unsub
  where
    Self: Sized,
  {
    self.raw_subscribe(Subscriber::local(SubscribeErr { next, error }))
  }
}
