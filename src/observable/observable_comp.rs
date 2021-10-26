use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverComp<N, C, Item> {
  next: N,
  complete: C,
  _marker: TypeHint<*const Item>,
}

impl<N, C, Item> Observer for ObserverComp<N, C, Item>
where
  C: FnMut(),
  N: FnMut(Item),
{
  type Item = Item;
  type Err = ();
  #[inline]
  fn next(&mut self, value: Item) { (self.next)(value); }
  #[inline]
  fn error(&mut self, _err: ()) {}
  fn complete(&mut self) { (self.complete)(); }
}

pub trait SubscribeComplete<'a, N, C> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N, C> SubscribeComplete<'a, N, C> for S
where
  S: LocalObservable<'a, Err = ()>,
  C: FnMut() + 'a,
  N: FnMut(S::Item) + 'a,
  S::Item: 'a,
{
  type Unsub = S::Unsub;
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
    S::Item: 'a,
  {
    let unsub = self.actual_subscribe(Subscriber::local(ObserverComp {
      next,
      complete,
      _marker: TypeHint::new(),
    }));
    SubscriptionWrapper(unsub)
  }
}

impl<'a, S, N, C> SubscribeComplete<'a, N, C> for Shared<S>
where
  S: SharedObservable<Err = ()>,
  C: FnMut() + Send + Sync + 'static,
  N: FnMut(S::Item) + Send + Sync + 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  fn subscribe_complete(
    self,
    next: N,
    complete: C,
  ) -> SubscriptionWrapper<Self::Unsub>
  where
    Self: Sized,
  {
    let unsub = self.0.actual_subscribe(Subscriber::shared(ObserverComp {
      next,
      complete,
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
        .subscribe_complete(|_| times += 1, || {})
        .unsubscribe_when_dropped();
    } // <-- guard is dropped here!
    subject.next(());
  }
  assert_eq!(times, 0);
}
