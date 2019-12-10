use crate::prelude::*;
use crate::subject::{LocalObserver, SharedSubject};

pub struct ConnectableObservable<Source, Subject> {
  source: Source,
  subject: Subject,
}

pub trait Connect {
  type Unsub;
  fn connect(self) -> Self::Unsub;
}

impl<S, O, U, Subscriber> RawSubscribable<Subscriber>
  for ConnectableObservable<S, Subject<O, U>>
where
  Subject<O, U>: RawSubscribable<Subscriber>,
{
  type Unsub = <Subject<O, U> as RawSubscribable<Subscriber>>::Unsub;

  #[inline(always)]
  fn raw_subscribe(self, subscriber: Subscriber) -> Self::Unsub {
    self.subject.raw_subscribe(subscriber)
  }
}

impl<S, P>
  ConnectableObservable<S, Subject<LocalObserver<P>, LocalSubscription>>
{
  pub fn local(observable: S) -> Self {
    Self {
      source: observable,
      subject: Subject::local_new(),
    }
  }
}

impl<Source, Item, Err> ConnectableObservable<Source, SharedSubject<Item, Err>>
where
  Source: IntoShared,
{
  pub fn shared(
    observable: Source,
  ) -> ConnectableObservable<Source::Shared, SharedSubject<Item, Err>> {
    ConnectableObservable {
      source: observable.to_shared(),
      subject: Subject::shared(),
    }
  }
}

impl<Source, O, U> Connect for ConnectableObservable<Source, Subject<O, U>>
where
  Source: RawSubscribable<Subscriber<O, U>>,
{
  type Unsub = Source::Unsub;
  fn connect(self) -> Self::Unsub {
    self.source.raw_subscribe(Subscriber {
      observer: self.subject.observers,
      subscription: self.subject.subscription,
    })
  }
}

impl<Source, Subject> IntoShared for ConnectableObservable<Source, Subject>
where
  Source: IntoShared,
  Subject: IntoShared,
{
  type Shared = ConnectableObservable<Source::Shared, Subject::Shared>;
  fn to_shared(self) -> Self::Shared {
    ConnectableObservable {
      source: self.source.to_shared(),
      subject: self.subject.to_shared(),
    }
  }
}

impl<Source, Subject> Fork for ConnectableObservable<Source, Subject>
where
  Subject: Fork,
{
  type Output = Subject::Output;
  #[inline(always)]
  fn fork(&self) -> Self::Output { self.subject.fork() }
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
    connected.fork().subscribe(|v| first = v);
    connected.fork().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn fork_and_shared() {
    let o = observable::of(100);
    let connected = ConnectableObservable::local(o).to_shared();
    connected.fork().subscribe(|_| {});
    connected.fork().subscribe(|_| {});

    connected.connect();
  }
}
