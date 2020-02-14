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

pub trait SubscribeErr<'a, N, E> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// completion.
  fn subscribe_err(self, next: N, error: E)
  -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N, E> SubscribeErr<'a, N, E> for S
where
  S: Observable<'a>,
  N: FnMut(S::Item) + 'a,
  E: FnMut(S::Err) + 'a,
{
  type Unsub = S::Unsub;
  fn subscribe_err(
    self,
    next: N,
    error: E,
  ) -> SubscriptionWrapper<Self::Unsub> {
    let unsub =
      self.actual_subscribe(Subscriber::local(ObserverErr { next, error }));
    SubscriptionWrapper(unsub)
  }
}

impl<'a, S, N, E> SubscribeErr<'a, N, E> for Shared<S>
where
  S: SharedObservable,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub = self
      .0
      .actual_subscribe(Subscriber::shared(ObserverErr { next, error }));
    SubscriptionWrapper(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::local();
    subject
      .clone()
      .subscribe_err(|_| times += 1, |_| {})
      .unsubscribe_when_dropped();
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
