use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverN<N>(N);

impl<Item, N> Observer<Item, ()> for ObserverN<N>
where
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.0)(value); }
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
  #[inline(always)]
  fn complete(&mut self) {}
}

pub trait SubscribeNext<'a, N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub>;
}

impl<'a, S, N> SubscribeNext<'a, N> for S
where
  S: Observable<'a, Err = ()>,
  N: FnMut(S::Item) + 'a,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub> {
    let unsub = self.actual_subscribe(Subscriber::local(ObserverN(next)));
    SubscriptionGuard(unsub)
  }
}

impl<'a, S, N> SubscribeNext<'a, N> for Shared<S>
where
  S: SharedObservable<Err = ()>,
  N: FnMut(S::Item) + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> SubscriptionGuard<Self::Unsub> {
    let unsub = self.0.actual_subscribe(Subscriber::shared(ObserverN(next)));
    SubscriptionGuard(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.clone().subscribe(|_| {
      times += 1;
    });
    subject.next(());
  }
  assert_eq!(times, 0);
}
