use crate::observer::{ObserverComplete, ObserverError, ObserverNext};
use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverN<N>(N);

impl<N> ObserverComplete for ObserverN<N> {
  #[inline(always)]
  fn complete(&mut self) {}
}

impl<N> ObserverError<()> for ObserverN<N> {
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
}

impl<N, Item> ObserverNext<Item> for ObserverN<N>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.0)(value); }
}

impl<N> IntoShared for ObserverN<N>
where
  N: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

pub trait SubscribeNext<N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> SubscriptionWrapper<Self::Unsub>;
}

impl<S, N> SubscribeNext<N> for S
where
  S: Observable<ObserverN<N>, LocalSubscription>,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub = self.actual_subscribe(Subscriber::local(ObserverN(next)));
    SubscriptionWrapper(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.fork().subscribe(|_| {
      times += 1;
    }).unsubscribe_when_dropped();
    subject.next(());
  }
  assert_eq!(times, 0);
}
