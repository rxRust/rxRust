use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverComp<N, C> {
  next: N,
  complete: C,
}

impl<N, C, Item> Observer<Item, ()> for ObserverComp<N, C>
where
  C: FnMut(),
  N: FnMut(Item),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
  #[inline(always)]
  fn error(&mut self, _err: ()) {}
  #[inline(always)]
  fn complete(&mut self) { (self.complete)(); }
}

impl<N, C> ObserverComp<N, C> {
  #[inline(always)]
  pub fn new(next: N, complete: C) -> Self { ObserverComp { next, complete } }
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

impl<'a, S, N, C> SubscribeComplete<N, C> for S
where
  S: Observable<'a, Err = ()>,
  C: FnMut() + 'a,
  N: FnMut(S::Item) + 'a,
{
  type Unsub = LocalSubscription;
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

impl<S, N, C> SubscribeComplete<N, C> for Shared<S>
where
  S: SharedObservable<Err = ()> + Send + Sync + 'static,
  C: FnMut() + Send + Sync + 'static,
  N: FnMut(S::Item) + Send + Sync + 'static,
{
  type Unsub = SharedSubscription;
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionGuard<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub =
      self
        .0
        .shared_actual_subscribe(Subscriber::shared(ObserverComp {
          next,
          complete,
        }));
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
