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

pub trait SubscribeNext<N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub>;
}

impl<'a, S, N> SubscribeNext<N> for S
where
  S: Observable<'a, Err = ()>,
  N: FnMut(S::Item) + 'a,
{
  type Unsub = LocalSubscription;
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub> {
    let unsub = self.actual_subscribe(Subscriber::local(ObserverN(next)));
    SubscriptionGuard(unsub)
  }
}

impl<'a, S, N> SubscribeNext<N> for Shared<S>
where
  S: SharedObservable<Err = ()> + Send + Sync + 'static,
  N: FnMut(S::Item) + Send + Sync + 'static,
{
  type Unsub = SharedSubscription;
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub> {
    let unsub = self
      .0
      .shared_actual_subscribe(Subscriber::shared(ObserverN(next)));
    SubscriptionGuard(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.fork().subscribe(|_| {
      times += 1;
    });
    subject.next(());
  }
  assert_eq!(times, 0);
}
