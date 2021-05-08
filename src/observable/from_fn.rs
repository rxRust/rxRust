use crate::prelude::*;

/// param `subscribe`: the function that is called when the Observable is
/// initially subscribed to. This function is given a Subscriber, to which
/// new values can be `next`ed, or an `error` method can be called to raise
/// an error, or `complete` can be called to notify of a successful
/// completion.
pub fn create<F, O, U, Item, Err>(
  subscribe: F,
) -> ObservableBase<FnEmitter<F, Item, Err>>
where
  F: FnOnce(Subscriber<O, U>),
  O: Observer<Item = Item, Err = Err>,
  U: SubscriptionLike,
{
  ObservableBase::new(FnEmitter(subscribe, TypeHint::new()))
}

#[derive(Clone)]
pub struct FnEmitter<F, Item, Err>(F, TypeHint<(Item, Err)>);

impl<F, Item, Err> Emitter for FnEmitter<F, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, F, Item, Err> LocalEmitter<'a> for FnEmitter<F, Item, Err>
where
  F: FnOnce(
    Subscriber<
      Box<dyn Observer<Item = Item, Err = Err> + 'a>,
      Box<dyn SubscriptionLike + 'a>,
    >,
  ),
{
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    (self.0)(Subscriber {
      observer: Box::new(subscriber.observer),
      subscription: Box::new(subscriber.subscription),
    })
  }
}

impl<F, Item, Err> SharedEmitter for FnEmitter<F, Item, Err>
where
  F: FnOnce(
    Subscriber<
      Box<dyn Observer<Item = Item, Err = Err> + Send + Sync + 'static>,
      SharedSubscription,
    >,
  ),
{
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    (self.0)(Subscriber {
      observer: Box::new(subscriber.observer),
      subscription: subscriber.subscription,
    })
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use bencher::Bencher;
  use std::sync::{Arc, Mutex};

  #[test]
  fn proxy_call() {
    let next = Arc::new(Mutex::new(0));
    let err = Arc::new(Mutex::new(0));
    let complete = Arc::new(Mutex::new(0));
    let c_next = next.clone();
    let c_err = err.clone();
    let c_complete = complete.clone();

    observable::create(|mut subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
      subscriber.next(&3);
      subscriber.error("never dispatch error");
    })
    .into_shared()
    .subscribe_all(
      move |_| *next.lock().unwrap() += 1,
      move |_: &str| *err.lock().unwrap() += 1,
      move || *complete.lock().unwrap() += 1,
    );

    assert_eq!(*c_next.lock().unwrap(), 3);
    assert_eq!(*c_complete.lock().unwrap(), 1);
    assert_eq!(*c_err.lock().unwrap(), 0);
  }
  #[test]
  fn support_fork() {
    let o = observable::create(|mut subscriber| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.next(&4);
    });
    let sum1 = Arc::new(Mutex::new(0));
    let sum2 = Arc::new(Mutex::new(0));
    let c_sum1 = sum1.clone();
    let c_sum2 = sum2.clone();
    o.clone().subscribe(move |v| *sum1.lock().unwrap() += v);
    o.clone().subscribe(move |v| *sum2.lock().unwrap() += v);

    assert_eq!(*c_sum1.lock().unwrap(), 10);
    assert_eq!(*c_sum2.lock().unwrap(), 10);
  }

  #[test]
  fn fork_and_share() {
    let observable = observable::create(|_| {});
    observable.clone().into_shared().subscribe(|_: i32| {});
    observable.clone().into_shared().subscribe(|_| {});

    let observable = observable::create(|_| {}).into_shared();
    observable.clone().subscribe(|_: i32| {});
    observable.clone().subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_from_fn);

  fn bench_from_fn(b: &mut Bencher) { b.iter(proxy_call); }
}
