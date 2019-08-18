use crate::prelude::*;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

/// combine two Observables into one by merging their emissions
///
/// # Example
///
/// ```
/// # use rxrust::{ ops::{Filter, Merge}, prelude::*};
/// let numbers = Subject::<i32, ()>::new();
/// // crate a even stream by filter
/// let even = numbers.fork().filter(|v| *v % 2 == 0);
/// // crate an odd stream by filter
/// let odd = numbers.fork().filter(|v| *v % 2 != 0);
///
/// // merge odd and even stream again
/// let merged = even.merge(odd);
///
/// // attach observers
/// merged.subscribe(|v| println!("{} ", v));
/// ```
pub trait Merge {
  fn merge<S>(self, o: S) -> MergeOp<Self, S>
  where
    Self: Sized,
  {
    MergeOp {
      source1: self,
      source2: o,
    }
  }
}

impl<O> Merge for O where O: RawSubscribable {}

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

impl<S1, S2> RawSubscribable for MergeOp<S1, S2>
where
  S1: RawSubscribable,
  S2: RawSubscribable<Item = S1::Item, Err = S1::Err>,
{
  type Err = S1::Err;
  type Item = S1::Item;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let stopped = AtomicBool::new(false);
    let completed = AtomicBool::new(false);

    let merge_subscribe =
      move |v: RxValue<&'_ Self::Item, &'_ Self::Err>| match v {
        nv @ RxValue::Next(_) => subscribe.call((nv,)),
        ev @ RxValue::Err(_) => {
          if !stopped.load(Ordering::Relaxed) {
            stopped.store(true, Ordering::Relaxed);
            subscribe.call((ev,))
          } else {
            RxReturn::Continue
          }
        }
        RxValue::Complete => {
          if !stopped.load(Ordering::Relaxed)
            && completed.load(Ordering::Relaxed)
          {
            stopped.store(true, Ordering::Relaxed);
            subscribe.call((RxValue::Complete,))
          } else {
            completed.store(true, Ordering::Relaxed);
            RxReturn::Continue
          }
        }
      };

    let merge_subscribe = Arc::new(RxFnWrapper::new(merge_subscribe));

    let unsub1 = self.source1.raw_subscribe(merge_subscribe.clone());
    let unsub2 = self.source2.raw_subscribe(merge_subscribe);
    Box::new(MergeSubscription::new(unsub1, unsub2))
  }
}

pub struct MergeSubscription {
  subscription1: Box<dyn Subscription + Send + Sync>,
  subscription2: Box<dyn Subscription + Send + Sync>,
}

impl MergeSubscription {
  fn new(
    s1: Box<dyn Subscription + Send + Sync>,
    s2: Box<dyn Subscription + Send + Sync>,
  ) -> Self {
    MergeSubscription {
      subscription1: s1,
      subscription2: s2,
    }
  }
}

impl<'a> Subscription for MergeSubscription {
  fn unsubscribe(&mut self) {
    self.subscription1.unsubscribe();
    self.subscription2.unsubscribe();
  }
}

impl<S1, S2> Multicast for MergeOp<S1, S2>
where
  S1: Multicast,
  S2: Multicast<Item = S1::Item, Err = S1::Err>,
{
  type Output = MergeOp<S1::Output, S2::Output>;
  fn multicast(self) -> Self::Output {
    MergeOp {
      source1: self.source1.multicast(),
      source2: self.source2.multicast(),
    }
  }
}

impl<S1, S2> Fork for MergeOp<S1, S2>
where
  S1: Fork,
  S2: Fork<Item = S1::Item, Err = S1::Err>,
{
  type Output = MergeOp<S1::Output, S2::Output>;
  fn fork(&self) -> Self::Output {
    MergeOp {
      source1: self.source1.fork(),
      source2: self.source2.fork(),
    }
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Fork, Merge, Multicast},
    prelude::*,
  };
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn odd_even_merge() {
    // three collection to store streams emissions
    let odd_store = Arc::new(Mutex::new(vec![]));
    let even_store = Arc::new(Mutex::new(vec![]));
    let numbers_store = Arc::new(Mutex::new(vec![]));

    let c_odd_store = odd_store.clone();
    let c_even_store = even_store.clone();
    let c_numbers_store = numbers_store.clone();

    let numbers = Subject::<_, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.fork().filter(|v| v % 2 == 0).multicast();
    // enabling multiple observers for odd stream;
    let odd = numbers.fork().filter(|v| *v % 2 != 0).multicast();

    // merge odd and even stream again
    let merged = even.fork().merge(odd.fork());

    //  attach observers
    merged.subscribe(move |v| numbers_store.lock().unwrap().push(*v));
    odd.subscribe(move |v| odd_store.lock().unwrap().push(*v));
    even.subscribe(move |v| even_store.lock().unwrap().push(*v));

    (0..10).for_each(|v| {
      numbers.next(&v);
    });

    assert_eq!(*c_even_store.lock().unwrap(), vec![0, 2, 4, 6, 8]);
    assert_eq!(*c_odd_store.lock().unwrap(), vec![1, 3, 5, 7, 9]);
    assert_eq!(
      *c_numbers_store.lock().unwrap(),
      (0..10).collect::<Vec<_>>()
    );
  }

  #[test]
  fn merge_unsubscribe_work() {
    let numbers = Subject::<_, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0);
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0);

    even
      .merge(odd)
      .subscribe(|_| unreachable!("oh, unsubscribe not work."))
      .unsubscribe();

    numbers.next(&1);
  }

  #[test]
  fn completed_test() {
    let completed = Arc::new(AtomicBool::new(false));
    let c_clone = completed.clone();
    let mut even = Subject::<_, ()>::new();
    let mut odd = Subject::<_, ()>::new();

    even.clone().merge(odd.clone()).subscribe_complete(
      |_: &()| {},
      move || completed.store(true, Ordering::Relaxed),
    );

    even.complete();
    assert_eq!(c_clone.load(Ordering::Relaxed), false);
    odd.complete();
    assert_eq!(c_clone.load(Ordering::Relaxed), true);
    c_clone.store(false, Ordering::Relaxed);
    even.complete();
    assert_eq!(c_clone.load(Ordering::Relaxed), false);
  }

  #[test]
  fn error_test() {
    let completed = Arc::new(Mutex::new(0));
    let cc = completed.clone();
    let error = Arc::new(Mutex::new(0));
    let ec = error.clone();
    let mut even = Subject::new();
    let mut odd = Subject::new();

    even.clone().merge(odd.clone()).subscribe_err_complete(
      |_: &()| {},
      move |_| *error.lock().unwrap() += 1,
      move || *completed.lock().unwrap() += 1,
    );

    odd.error(&"");
    even.clone().error(&"");
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(*cc.lock().unwrap(), 0);
    // error should be hit just once
    assert_eq!(*ec.lock().unwrap(), 1);
  }

  #[test]
  fn merge_fork() {
    let o = Observable::new(|mut s| {
      s.next(&1);
      s.next(&2);
      s.error(&());
    })
    .multicast();

    let m = o.fork().merge(o.fork());
    m.fork().merge(m.fork());
  }
}
