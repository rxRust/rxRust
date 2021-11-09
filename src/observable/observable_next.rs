use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverN<N, Item> {
  next: N,
  is_stopped: bool,
  _marker: TypeHint<*const Item>,
}

impl<Item, N> Observer for ObserverN<N, Item>
where
  N: FnMut(Item),
{
  type Item = Item;
  type Err = ();
  fn next(&mut self, value: Self::Item) {
    if !self.is_stopped {
      (self.next)(value);
    }
  }
  #[inline]
  fn error(&mut self, _err: ()) { self.is_stopped = true; }
  #[inline]
  fn complete(&mut self) { self.is_stopped = true; }
}

pub trait SubscribeNext<'a, N> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: SubscriptionLike;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: N) -> SubscriptionWrapper<Self::Unsub>;
}

impl<'a, S, N> SubscribeNext<'a, N> for S
where
  S: LocalObservable<'a, Err = ()>,
  N: FnMut(S::Item) + 'a,
  S::Item: 'a,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> SubscriptionWrapper<Self::Unsub> {
    let unsub = self.actual_subscribe(ObserverN {
      next,
      is_stopped: false,
      _marker: TypeHint::new(),
    });
    SubscriptionWrapper(unsub)
  }
}

impl<'a, S, N> SubscribeNext<'a, N> for Shared<S>
where
  S: SharedObservable<Err = ()>,
  N: FnMut(S::Item) + Send + Sync + 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: N) -> SubscriptionWrapper<Self::Unsub> {
    let unsub = self.0.actual_subscribe(ObserverN {
      next,
      is_stopped: false,
      _marker: TypeHint::new(),
    });
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
        .subscribe(|_| {
          times += 1;
        })
        .unsubscribe_when_dropped();
    } // <-- guard is dropped here!
    subject.next(());
  }
  assert_eq!(times, 0);
}
