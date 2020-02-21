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

impl<Item, Err, N, E, C> Observer<Item, Err> for ObserverAll<N, E, C>
where
  C: FnMut(),
  N: FnMut(Item),
  E: FnMut(Err),
{
  #[inline(always)]
  fn next(&mut self, value: Item) { (self.next)(value); }
  #[inline(always)]
  fn error(&mut self, err: Err) { (self.error)(err); }
  #[inline(always)]
  fn complete(&mut self) { (self.complete)(); }
}

pub trait SubscribeAll<'a, N, E, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  fn subscribe_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N, E, C> SubscribeAll<'a, N, E, C> for S
where
  S: Observable<'a>,
  N: FnMut(S::Item) + 'a,
  E: FnMut(S::Err) + 'a,
  C: FnMut() + 'a,
{
  type Unsub = S::Unsub;
  fn subscribe_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let subscriber = Subscriber::local(ObserverAll {
      next,
      error,
      complete,
    });
    SubscriptionWrapper(self.actual_subscribe(subscriber))
  }
}

impl<'a, S, N, E, C> SubscribeAll<'a, N, E, C> for Shared<S>
where
  S: SharedObservable,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
  C: FnMut() + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_all(
    self,
    next: N,
    error: E,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let subscriber = Subscriber::shared(ObserverAll {
      next,
      error,
      complete,
    });
    SubscriptionWrapper(self.0.actual_subscribe(subscriber))
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::new();
    subject
      .clone()
      .subscribe_all(|_| times += 1, |_| {}, || {})
      .unsubscribe_when_dropped();
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
