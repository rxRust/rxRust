use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverAll<N, E, C> {
  next: N,
  error: E,
  complete: C,
}

impl<N, E, C> ObserverAll<N, E, C> {
  #[inline(always)]
  pub fn new(next: N, error: E, complete: C) -> Self {
    ObserverAll {
      next,
      error,
      complete,
    }
  }
}

impl<N, E, C> IntoShared for ObserverAll<N, E, C>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<N, E, C> ObserverComplete for ObserverAll<N, E, C>
where
  C: FnMut(),
{
  #[inline(always)]
  fn complete(&mut self) { (self.complete)(); }
}

impl<N, E, C, Err> ObserverError<Err> for ObserverAll<N, E, C>
where
  E: FnMut(Err),
{
  #[inline(always)]
  fn error(&mut self, err: Err) { (self.error)(err); }
}

impl<N, E, C, Item> ObserverNext<Item> for ObserverAll<N, E, C>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
}

pub trait SubscribeAll<N, E, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  ///
  fn subscribe_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionGuard<Self::Unsub>;
}

impl<S, N, E, C> SubscribeAll<N, E, C> for S
where
  S: Observable<ObserverAll<N, E, C>, LocalSubscription>,
{
  type Unsub = S::Unsub;
  fn subscribe_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionGuard<Self::Unsub>
  where
    Self: Sized,
  {
    let subscriber = Subscriber::local(ObserverAll {
      next,
      error,
      complete,
    });
    SubscriptionGuard(self.actual_subscribe(subscriber))
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.fork().subscribe_all(|_| times += 1, |_| {}, || {});
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
