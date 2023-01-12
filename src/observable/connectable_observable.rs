use crate::prelude::*;

pub struct ConnectableObservable<S, Subject> {
  source: S,
  subject: Subject,
}

impl<S, Subject, Item, Err, O> Observable<Item, Err, O>
  for ConnectableObservable<S, Subject>
where
  Subject: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  type Unsub = Subject::Unsub;

  #[inline]
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.subject.actual_subscribe(observer)
  }
}

impl<S, Subject> ConnectableObservable<S, Subject> {
  #[inline]
  pub fn new(source: S) -> Self
  where
    Subject: Default,
  {
    ConnectableObservable { source, subject: <_>::default() }
  }

  #[inline]
  pub fn fork(&self) -> Subject
  where
    Subject: Clone,
  {
    self.subject.clone()
  }

  #[inline]
  pub fn connect<Item, Err>(self) -> S::Unsub
  where
    S: Observable<Item, Err, Subject>,
    Subject: Observer<Item, Err>,
  {
    self.source.actual_subscribe(self.subject)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let o = observable::of(100);
    let connected = ConnectableObservable::<_, Subject<_, _>>::new(o);
    let mut first = 0;
    let mut second = 0;
    connected.fork().subscribe(|v| first = v);
    connected.fork().subscribe(|v| second = v);

    connected.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn publish_smoke() {
    let p = observable::of(100).publish::<Subject<'_, _, _>>();
    let mut first = 0;
    let mut second = 0;
    p.fork().subscribe(|v| first = v);
    p.fork().subscribe(|v| second = v);

    p.connect();
    assert_eq!(first, 100);
    assert_eq!(second, 100);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_connectable);

  fn bench_connectable(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }
}
