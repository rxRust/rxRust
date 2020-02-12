use crate::prelude::*;
use crate::subject::{LocalSubject, SharedSubject};

#[derive(Clone)]
pub struct ConnectableObservable<Source, Subject> {
  source: Source,
  subject: Subject,
}

pub trait Connect {
  type Unsub;
  fn connect(self) -> Self::Unsub;
}

macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  type Unsub = $subscription;
  #[inline(always)]
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.subject.actual_subscribe(subscriber)
  }
}

impl<'a, S, Item, Err> Observable<'a>
  for ConnectableObservable<S, LocalSubject<'a, Item, Err>>
where
  S: Observable<'a, Item = Item, Err = Err>,
  S: Observable<'a, Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  observable_impl!(LocalSubscription, 'a);
}

impl<S, Item, Err> SharedObservable
  for ConnectableObservable<S, SharedSubject<Item, Err>>
where
  S: SharedObservable<Item = Item, Err = Err>,
  S: SharedObservable<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

impl<'a, Item, Err, S> ConnectableObservable<S, LocalSubject<'a, Item, Err>>
where
  S: Observable<'a, Item = Item, Err = Err>,
{
  pub fn local(observable: S) -> Self {
    Self {
      source: observable,
      subject: Subject::local(),
    }
  }
}

impl<Source, Item, Err> ConnectableObservable<Source, SharedSubject<Item, Err>>
where
  Source: SharedObservable<Item = Item, Err = Err>,
{
  pub fn shared(observable: Source) -> Self {
    ConnectableObservable {
      source: observable,
      subject: Subject::shared(),
    }
  }
}

impl<'a, S, Item, Err> Connect
  for ConnectableObservable<S, LocalSubject<'a, Item, Err>>
where
  S: Observable<'a, Item = Item, Err = Err>,
  Item: Copy + 'a,
  Err: Copy + 'a,
{
  type Unsub = S::Unsub;
  fn connect(self) -> Self::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: self.subject.observers,
      subscription: self.subject.subscription,
    })
  }
}

impl<S, Item, Err> Connect
  for ConnectableObservable<S, SharedSubject<Item, Err>>
where
  S: SharedObservable<Item = Item, Err = Err>,
  Item: Copy + Send + Sync + 'static,
  Err: Copy + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  fn connect(self) -> Self::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: self.subject.observers,
      subscription: self.subject.subscription,
    })
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of(100);
    let connected = ConnectableObservable::local(o);
    let mut first = 0;
    let mut second = 0;
    let _guard1 = connected.clone().subscribe(|v| first = v);
    let _guard2 = connected.clone().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn fork_and_shared() {
    let o = observable::of(100);
    let connected = ConnectableObservable::shared(o);
    connected.clone().to_shared().subscribe(|_| {});
    connected.clone().to_shared().subscribe(|_| {});

    connected.connect();
  }
}
