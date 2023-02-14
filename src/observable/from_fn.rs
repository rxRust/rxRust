use crate::{prelude::*, subscriber::Subscriber};

/// param `func`: the function that is called when the Observable is
/// initially subscribed to. This function is given a `Observer`, to which
/// new values can be `next`ed, or an `error` method can be called to raise
/// an error, or `complete` can be called to notify of a successful
/// completion.
pub fn create<F, Item, Err, P>(func: F) -> ObservableFn<F, P>
where
  F: FnOnce(P),
  P: Observer<Item, Err> + Subscription,
{
  ObservableFn { func, _hint: TypeHint::default() }
}

pub struct ObservableFn<F, P> {
  func: F,
  _hint: TypeHint<P>,
}

macro_rules! impl_observable {
  ($subscriber:ident $($bounds: tt)*) => {
    impl<F, Item, Err, O> Observable<Item, Err, O>
      for ObservableFn<F, $subscriber<O>>
    where
      F: FnOnce($subscriber<O>),
      O: Observer<Item, Err> $($bounds)*
    {
      type Unsub = $subscriber<O>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let subscriber = $subscriber::new(Some(observer));
        (self.func)(subscriber.clone());
        subscriber
      }
    }

    impl<F, Item, Err, O> ObservableExt<Item, Err>
      for ObservableFn<F, $subscriber<O>>
    where
      F: FnOnce($subscriber<O>),
      O: Observer<Item, Err>{}
  };
}

impl_observable!(Subscriber);
impl_observable!(SubscriberThreads + Send + 'static);

impl<F, P> Clone for ObservableFn<F, P>
where
  F: Clone,
{
  #[inline]
  fn clone(&self) -> Self {
    Self {
      func: self.func.clone(),
      _hint: TypeHint::new(),
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
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

    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.complete();
    })
    .on_complete(move || *complete.lock().unwrap() += 1)
    .on_error(move |_: &str| *err.lock().unwrap() += 1)
    .subscribe(move |_| *next.lock().unwrap() += 1);

    assert_eq!(*c_next.lock().unwrap(), 3);
    assert_eq!(*c_complete.lock().unwrap(), 1);
    assert_eq!(*c_err.lock().unwrap(), 0);
  }
  #[test]
  fn support_fork() {
    let o = observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(&1);
      subscriber.next(&2);
      subscriber.next(&3);
      subscriber.next(&4);
    });
    let sum1 = Arc::new(Mutex::new(0));
    let sum2 = Arc::new(Mutex::new(0));
    let c_sum1 = sum1.clone();
    let c_sum2 = sum2.clone();
    let u1: Box<dyn FnMut(&i32) + Send> =
      Box::new(move |v| *sum1.lock().unwrap() += v);
    let u2: Box<dyn FnMut(&i32) + Send> =
      Box::new(move |v| *sum2.lock().unwrap() += v);
    o.clone().subscribe(u1);
    o.clone().subscribe(u2);

    assert_eq!(*c_sum1.lock().unwrap(), 10);
    assert_eq!(*c_sum2.lock().unwrap(), 10);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_from_fn);

  fn bench_from_fn(b: &mut Bencher) {
    b.iter(proxy_call);
  }
}
