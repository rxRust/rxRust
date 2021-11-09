use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverAll<N, E, C, Item, Err> {
  next: N,
  error: E,
  complete: C,
  is_stopped: bool,
  _marker: TypeHint<(*const Item, *const Err)>,
}

impl<Item, Err, N, E, C> Observer for ObserverAll<N, E, C, Item, Err>
where
  C: FnMut(),
  N: FnMut(Item),
  E: FnMut(Err),
{
  type Item = Item;
  type Err = Err;
  #[inline(always)]
  fn next(&mut self, value: Self::Item) {
    if !self.is_stopped {
      (self.next)(value);
    }
  }

  fn error(&mut self, err: Self::Err) {
    if !self.is_stopped {
      (self.error)(err);
      self.is_stopped = true;
    }
  }

  fn complete(&mut self) {
    if !self.is_stopped {
      (self.complete)();
      self.is_stopped = true;
    }
  }
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
  S: LocalObservable<'a>,
  N: FnMut(S::Item) + 'a,
  E: FnMut(S::Err) + 'a,
  S::Err: 'a,
  S::Item: 'a,
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
    SubscriptionWrapper(self.actual_subscribe(ObserverAll {
      next,
      error,
      complete,
      is_stopped: false,
      _marker: TypeHint::new(),
    }))
  }
}

impl<'a, S, N, E, C> SubscribeAll<'a, N, E, C> for Shared<S>
where
  S: SharedObservable,
  N: FnMut(S::Item) + Send + Sync + 'static,
  E: FnMut(S::Err) + Send + Sync + 'static,
  C: FnMut() + Send + Sync + 'static,
  S::Err: 'static,
  S::Item: 'static,
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
    SubscriptionWrapper(self.0.actual_subscribe(ObserverAll {
      next,
      error,
      complete,
      is_stopped: false,
      _marker: TypeHint::new(),
    }))
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
        .subscribe_all(|_| times += 1, |_| {}, || {})
        .unsubscribe_when_dropped();
    } // <-- guard is dropped here!
    subject.next(());
    subject.error(());
  }
  assert_eq!(times, 0);
}
