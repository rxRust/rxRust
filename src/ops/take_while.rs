use crate::prelude::*;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct TakeWhileOp<S, F> {
  pub(crate) source: S,
  pub(crate) callback: F,
}

impl<S, F> Observable for TakeWhileOp<S, F>
where
  S: Observable,
  F: FnMut(&S::Item) -> bool,
{
  type Item = S::Item;
  type Err = S::Err;
}

impl<'a, S, F> LocalObservable<'a> for TakeWhileOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut(&S::Item) -> bool + 'a,
  S::Unsub: 'a,
{
  type Unsub = Rc<RefCell<ProxySubscription<S::Unsub>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    let subscription = Rc::new(RefCell::new(ProxySubscription::default()));
    let observer = TakeWhileObserver {
      observer,
      subscription: subscription.clone(),
      callback: self.callback,
    };
    subscription
      .borrow_mut()
      .proxy(self.source.actual_subscribe(observer));
    subscription
  }
}

impl<S, F> SharedObservable for TakeWhileOp<S, F>
where
  S: SharedObservable,
  F: FnMut(&S::Item) -> bool + Send + Sync + 'static,
{
  type Unsub = Arc<Mutex<ProxySubscription<S::Unsub>>>;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    let subscription = Arc::new(Mutex::new(ProxySubscription::default()));
    let observer = TakeWhileObserver {
      observer,
      subscription: subscription.clone(),
      callback: self.callback,
    };
    subscription
      .lock()
      .unwrap()
      .proxy(self.source.actual_subscribe(observer));
    subscription
  }
}

pub struct TakeWhileObserver<O, S, F> {
  observer: O,
  subscription: S,
  callback: F,
}

impl<O, U, Item, Err, F> Observer for TakeWhileObserver<O, U, F>
where
  O: Observer<Item = Item, Err = Err>,
  U: SubscriptionLike,
  F: FnMut(&Item) -> bool,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if (self.callback)(&value) {
      self.observer.next(value);
    } else {
      self.observer.complete();
      self.subscription.unsubscribe();
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
      .take_while(|v| v < &5)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn take_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take_while5 = observable::from_iter(0..100).take_while(|v| v < &5);
      let f1 = take_while5.clone();
      let f2 = take_while5;

      f1.take_while(|v| v < &5).subscribe(|_| nc1 += 1);
      f2.take_while(|v| v < &5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn ininto_shared() {
    observable::from_iter(0..100)
      .take_while(|v| v < &5)
      .take_while(|v| v < &5)
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take_while);

  fn bench_take_while(b: &mut bencher::Bencher) { b.iter(base_function); }
}
