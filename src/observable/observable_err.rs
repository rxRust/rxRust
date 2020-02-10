use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverErr<N, E> {
  next: N,
  error: E,
}

impl<Item, Err, N, E> Observer<Item, Err> for ObserverErr<N, E>
where
  N: FnMut(Item),
  E: FnMut(Err),
{
  #[inline(always)]
  fn next(&mut self, err: Item) { (self.next)(err); }
  #[inline(always)]
  fn error(&mut self, err: Err) { (self.error)(err); }
  #[inline(always)]
  fn complete(&mut self) {}
}

impl<N, E> ObserverErr<N, E> {
  #[inline(always)]
  pub fn new(next: N, error: E) -> Self { ObserverErr { next, error } }
}

pub trait SubscribeErr<N, E> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// completion.
  ///
  fn subscribe_err(self, next: N, error: E) -> SubscriptionGuard<Self::Unsub>;
}

impl<'a, S, N, E> SubscribeErr<N, E> for S
where
  S: Observable<'a>,
  N: FnMut(S::Item) + 'a,
  E: FnMut(S::Err) + 'a,
{
  type Unsub = LocalSubscription;
  fn subscribe_err(self, next: N, error: E) -> SubscriptionGuard<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub =
      self.actual_subscribe(Subscriber::local(ObserverErr { next, error }));
    SubscriptionGuard(unsub)
  }
}

impl<S, N, E> SubscribeErr<N, E> for Shared<S>
where
  S: SharedObservable + Send + Sync + 'static,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
{
  type Unsub = SharedSubscription;
  fn subscribe_err(self, next: N, error: E) -> SubscriptionGuard<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub = self
      .0
      .shared_actual_subscribe(Subscriber::shared(ObserverErr { next, error }));
    SubscriptionGuard(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject.fork().subscribe_err(|_| times += 1, |_| {});
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
