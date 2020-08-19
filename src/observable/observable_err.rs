use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverErr<N, E> {
  next: N,
  error: E,
  is_stopped: bool,
}

impl<Item, Err, N, E> Observer<Item, Err> for ObserverErr<N, E>
where
  N: FnMut(Item),
  E: FnMut(Err),
{
  #[inline]
  fn next(&mut self, err: Item) { (self.next)(err); }
  fn error(&mut self, err: Err) {
    (self.error)(err);
    self.is_stopped = true;
  }
  #[inline]
  fn complete(&mut self) { self.is_stopped = true; }
  #[inline]
  fn is_stopped(&self) -> bool { self.is_stopped }
}

impl<N, E> ObserverErr<N, E> {
  #[inline(always)]
  pub fn new(next: N, error: E) -> Self {
    ObserverErr {
      next,
      error,
      is_stopped: false,
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
      is_stopped: false,
    }));
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
    let unsub = self.0.actual_subscribe(Subscriber::shared(ObserverErr {
      next,
      error,
      is_stopped: false,
    }));
    SubscriptionWrapper(unsub)
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::new();
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
