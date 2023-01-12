use crate::prelude::*;

#[derive(Clone)]
pub struct ObserverItem<N> {
  next: N,
}

impl<Item, N> Observer<Item, ()> for ObserverItem<N>
where
  N: FnMut(Item),
{
  fn next(&mut self, value: Item) {
    (self.next)(value);
  }

  #[inline]
  fn error(self, _err: ()) {}

  #[inline]
  fn complete(self) {}

  #[inline]
  fn is_finished(&self) -> bool {
    false
  }
}

pub trait ObservableItem<Item, F> {
  /// A type implementing [`SubscriptionLike`]
  type Unsub: Subscription;

  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  fn subscribe(self, next: F) -> Self::Unsub;
}

impl<S, Item, F> ObservableItem<Item, F> for S
where
  S: Observable<Item, (), ObserverItem<F>>,
  F: FnMut(Item),
{
  type Unsub = S::Unsub;
  fn subscribe(self, next: F) -> Self::Unsub {
    self.actual_subscribe(ObserverItem { next })
  }
}

#[test]
fn raii() {
  let mut times = 0;
  {
    let mut subject = Subject::default();
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
