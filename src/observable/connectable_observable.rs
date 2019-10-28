use crate::prelude::*;

pub struct ConnectableObservable<O, S> {
  source: O,
  subject: S,
}

impl<Item, Err, Sub, O, SO, U> RawSubscribable<Item, Err, Sub>
  for ConnectableObservable<O, Subject<SO, U>>
where
  Subject<SO, U>: RawSubscribable<Item, Err, Sub>,
{
  type Unsub = <Subject<SO, U> as RawSubscribable<Item, Err, Sub>>::Unsub;

  #[inline(always)]
  fn raw_subscribe(self, subscriber: Sub) -> Self::Unsub {
    self.subject.raw_subscribe(subscriber)
  }
}

impl<'a, Item, Err, O>
  ConnectableObservable<
    O,
    Subject<subject::LocalPublishers<'a, Item, Err>, LocalSubscription>,
  >
{
  pub fn local(observable: O) -> Self {
    ConnectableObservable {
      source: observable,
      subject: Subject::local(),
    }
  }
}

impl<O, Item, Err>
  ConnectableObservable<
    O,
    Subject<subject::SharedPublishers<Item, Err>, SharedSubscription>,
  >
where
  O: IntoShared,
{
  pub fn shared(
    observable: O,
  ) -> ConnectableObservable<
    O::Shared,
    Subject<subject::SharedPublishers<Item, Err>, SharedSubscription>,
  > {
    ConnectableObservable {
      source: observable.to_shared(),
      subject: Subject::shared(),
    }
  }
}

impl<O, SO, U> ConnectableObservable<O, Subject<SO, U>> {
  pub fn connect<Item, Err>(self) -> O::Unsub
  where
    O: RawSubscribable<Item, Err, Subscriber<Subject<SO, U>, U>>,
    Subject<SO, U>: Fork<Output = Subject<SO, U>>,
  {
    self.source.raw_subscribe(Subscriber {
      observer: self.subject.fork(),
      subscription: self.subject.subscription,
    })
  }
}

impl<O, S> IntoShared for ConnectableObservable<O, S>
where
  O: IntoShared,
  S: IntoShared,
{
  type Shared = ConnectableObservable<O::Shared, S::Shared>;
  fn to_shared(self) -> Self::Shared {
    ConnectableObservable {
      source: self.source.to_shared(),
      subject: self.subject.to_shared(),
    }
  }
}

impl<O, S> Fork for ConnectableObservable<O, S>
where
  O: Fork,
  S: Fork,
{
  type Output = ConnectableObservable<O::Output, S::Output>;
  fn fork(&self) -> Self::Output {
    ConnectableObservable {
      source: self.source.fork(),
      subject: self.subject.fork(),
    }
  }
}
#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of!(100);
    let connected = ConnectableObservable::local(o);
    let mut first = 0;
    let mut second = 0;
    connected.fork().subscribe(|v| first = *v);
    connected.fork().subscribe(|v| second = *v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn fork_and_shared() {
    let o = observable::of!(100);
    let connected = ConnectableObservable::local(o)
      .to_shared()
      .fork()
      .to_shared();
    connected.fork().subscribe(|_| {});
    connected.fork().subscribe(|_| {});

    connected.connect();
  }
}
