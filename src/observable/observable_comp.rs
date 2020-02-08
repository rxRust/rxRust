use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverComp<N, C> {
  next: N,
  complete: C,
}

impl<N, C> ObserverComplete for ObserverComp<N, C>
where
  C: FnMut(),
{
  #[inline(always)]
  fn complete(&mut self) { (self.complete)(); }
}

impl<N, C> ObserverError<()> for ObserverComp<N, C> {
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
}

impl<N, C, Item> ObserverNext<Item> for ObserverComp<N, C>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
}

impl<N, C> ObserverComp<N, C> {
  #[inline(always)]
  pub fn new(next: N, complete: C) -> Self { ObserverComp { next, complete } }
}

impl<N, C> IntoShared for ObserverComp<N, C>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribeComplete<N, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionGuard<Self::Unsub>;
}

impl<S, N, C> SubscribeComplete<N, C> for S
where
  S: Observable<ObserverComp<N, C>, LocalSubscription>,
  C: FnMut(),
{
  type Unsub = S::Unsub;
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionGuard<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub =
      self.actual_subscribe(Subscriber::local(ObserverComp { next, complete }));
    SubscriptionGuard(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.fork().subscribe_complete(|_| times += 1, || {});
    subject.next(());
  }
  assert_eq!(times, 0);
}
