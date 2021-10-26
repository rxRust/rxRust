use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverErr<N, E, Item, Err> {
  next: N,
  error: E,
  _marker: TypeHint<fn() -> (Item, Err)>,
}

impl<Item, Err, N, E> Observer for ObserverErr<N, E, Item, Err>
where
  N: FnMut(Item),
  E: FnMut(Err),
{
  type Item = Item;
  type Err = Err;
  #[inline]
  fn next(&mut self, err: Item) { (self.next)(err); }
  fn error(&mut self, err: Err) { (self.error)(err); }
  #[inline]
  fn complete(&mut self) {}
}

impl<N, E, Item, Err> ObserverErr<N, E, Item, Err> {
  #[inline(always)]
  pub fn new(next: N, error: E) -> Self {
    ObserverErr {
      next,
      error,
      _marker: TypeHint::new(),
    }
  }
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
  S: LocalObservable<'a>,
  N: FnMut(S::Item) + 'a,
  E: FnMut(S::Err) + 'a,
  S::Err: 'a,
  S::Item: 'a,
{
  type Unsub = S::Unsub;
  fn subscribe_err(
    self,
    next: N,
    error: E,
  ) -> SubscriptionWrapper<Self::Unsub> {
    let unsub = self.actual_subscribe(Subscriber::local(ObserverErr {
      next,
      error,
      _marker: TypeHint::new(),
    }));
    SubscriptionWrapper(unsub)
  }
}

impl<'a, S, N, E> SubscribeErr<'a, N, E> for Shared<S>
where
  S: SharedObservable,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
  S::Item: 'static,
  S::Err: 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_err(self, next: N, error: E) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub = self.0.actual_subscribe(Subscriber::shared(ObserverErr {
      next,
      error,
      _marker: TypeHint::new(),
    }));
    SubscriptionWrapper(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = LocalSubject::new();
    {
      let _ = subject
        .clone()
        .subscribe_err(|_| times += 1, |_| {})
        .unsubscribe_when_dropped();
    } // <-- guard is dropped here!
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
