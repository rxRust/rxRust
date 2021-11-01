use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

use crate::prelude::*;

#[derive(Clone)]
pub struct TakeOp<S> {
  pub(crate) source: S,
  pub(crate) count: u32,
}

observable_proxy_impl!(TakeOp, S);

impl<'a, S> LocalObservable<'a> for TakeOp<S>
where
  S: LocalObservable<'a>,
  S::Unsub: 'static,
{
  type Unsub = Rc<RefCell<ProxySubscription<S::Unsub>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    let subscription = Rc::new(RefCell::new(ProxySubscription::default()));
    let observer = TakeObserver {
      observer,
      subscription: subscription.clone(),
      count: self.count,
      hits: 0,
    };
    subscription
      .borrow_mut()
      .proxy(self.source.actual_subscribe(observer));
    subscription
  }
}

impl<S> SharedObservable for TakeOp<S>
where
  S: SharedObservable,
{
  type Unsub = Arc<Mutex<ProxySubscription<S::Unsub>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let subscription = Arc::new(Mutex::new(ProxySubscription::default()));
    let observer = TakeObserver {
      observer,
      subscription: subscription.clone(),
      count: self.count,
      hits: 0,
    };
    subscription
      .lock()
      .unwrap()
      .proxy(self.source.actual_subscribe(observer));
    subscription
  }
}

pub struct TakeObserver<O, S> {
  observer: O,
  subscription: S,
  count: u32,
  hits: u32,
}

impl<O, U, Item, Err> Observer for TakeObserver<O, U>
where
  O: Observer<Item = Item, Err = Err>,
  U: SubscriptionLike,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if self.hits < self.count {
      self.hits += 1;
      self.observer.next(value);
      if self.hits == self.count {
        self.complete();
        self.subscription.unsubscribe();
      }
    }
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take(5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn take_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take5 = observable::from_iter(0..100).take(5);
      let f1 = take5.clone();
      let f2 = take5;

      f1.take(5).subscribe(|_| nc1 += 1);
      f2.take(5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn ininto_shared() {
    observable::from_iter(0..100)
      .take(5)
      .take(5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take);

  fn bench_take(b: &mut bencher::Bencher) { b.iter(base_function); }
}
