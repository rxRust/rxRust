use crate::observer::next_proxy_impl;
use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// combine two Observables into one by merging their emissions
///
/// # Example
///
/// ```
/// # use rxrust::{ ops::{Filter, Merge}, prelude::*};
/// let numbers = Subject::new();
/// // crate a even stream by filter
/// let even = numbers.clone().filter(|v| *v % 2 == 0);
/// // crate an odd stream by filter
/// let odd = numbers.clone().filter(|v| *v % 2 != 0);
///
/// // merge odd and even stream again
/// let merged = even.merge(odd);
///
/// // attach observers
/// merged.subscribe(|v: &i32| println!("{} ", v));
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

impl<O> Merge for O {}

#[derive(Clone)]
pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

pub struct SharedMergeOp<S1, S2>(MergeOp<S1, S2>);

impl<'a, S1, S2> Observable<'a> for MergeOp<S1, S2>
where
  S1: Observable<'a>,
  S2: Observable<'a, Item = S1::Item, Err = S1::Err>,
{
  type Item = S1::Item;
  type Err = S1::Err;
  type Unsub = LocalSubscription;

  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let mut subscription = subscriber.subscription;
    let merge_observer = Rc::new(RefCell::new(MergeObserver {
      observer: subscriber.observer,
      subscription: subscription.clone(),
      completed_one: false,
    }));
    subscription.add(self.source1.actual_subscribe(Subscriber {
      observer: merge_observer.clone(),
      subscription: LocalSubscription::default(),
    }));
    subscription.add(self.source2.actual_subscribe(Subscriber {
      observer: merge_observer,
      subscription: LocalSubscription::default(),
    }));
    subscription
  }
}

impl<S1, S2> SharedObservable for MergeOp<S1, S2>
where
  S1: SharedObservable,
  S2: SharedObservable<Item = S1::Item, Err = S1::Err, Unsub = S1::Unsub>,
  S1::Unsub: Send + Sync,
{
  type Item = S1::Item;
  type Err = S1::Err;
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let mut subscription = subscriber.subscription;
    let merge_observer = Arc::new(Mutex::new(MergeObserver {
      observer: subscriber.observer,
      subscription: subscription.clone(),
      completed_one: false,
    }));
    subscription.add(self.source1.actual_subscribe(Subscriber {
      observer: merge_observer.clone(),
      subscription: SharedSubscription::default(),
    }));
    subscription.add(self.source2.actual_subscribe(Subscriber {
      observer: merge_observer,
      subscription: SharedSubscription::default(),
    }));
    subscription
  }
}

#[derive(Clone)]
pub struct MergeObserver<O, Unsub> {
  observer: O,
  subscription: Unsub,
  completed_one: bool,
}

impl<Item, Err, O, Unsub> Observer<Item, Err> for MergeObserver<O, Unsub>
where
  O: Observer<Item, Err>,
  Unsub: SubscriptionLike,
{
  next_proxy_impl!(Item, observer);
  fn error(&mut self, err: Err) {
    self.observer.error(err);
    self.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    if self.completed_one {
      self.observer.complete();
      self.subscription.unsubscribe();
    } else {
      self.completed_one = true;
    }
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Merge},
    prelude::*,
  };
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn odd_even_merge() {
    // three collection to store streams emissions
    let mut odd_store = vec![];
    let mut even_store = vec![];
    let mut numbers_store = vec![];

    {
      let mut numbers = Subject::new();
      // enabling multiple observers for even stream;
      let even = numbers.clone().filter(|v| *v % 2 == 0);
      // enabling multiple observers for odd stream;
      let odd = numbers.clone().filter(|v| *v % 2 != 0);

      // merge odd and even stream again
      let merged = even.clone().merge(odd.clone());

      //  attach observers
      merged.subscribe(|v| numbers_store.push(v));
      odd.subscribe(|v| odd_store.push(v));
      even.subscribe(|v| even_store.push(v));

      (0..10).for_each(|v| {
        numbers.next(v);
      });
    }
    assert_eq!(even_store, vec![0, 2, 4, 6, 8]);
    assert_eq!(odd_store, vec![1, 3, 5, 7, 9]);
    assert_eq!(numbers_store, (0..10).collect::<Vec<_>>());
  }

  #[test]
  fn merge_unsubscribe_work() {
    let mut numbers = Subject::new();
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
    let mut even = Subject::new();
    let mut odd = Subject::new();

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

    even.clone().merge(odd.clone()).subscribe_all(
      |_: ()| {},
      move |_| *error.lock().unwrap() += 1,
      move || *completed.lock().unwrap() += 1,
    );

    odd.error("");
    even.clone().error("");
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(*cc.lock().unwrap(), 0);
    // error should be hit just once
    assert_eq!(*ec.lock().unwrap(), 1);
  }

  #[test]
  fn merge_fork() {
    let o = observable::create(|mut s| {
      s.next(1);
      s.next(2);
      s.error(());
    });

    let m = o.clone().merge(o.clone());
    m.clone().merge(m.clone()).subscribe(|_| {});
  }

  #[test]
  fn merge_local_and_shared() {
    let mut res = vec![];
    let shared = observable::of(1);
    let local = observable::of(2);

    shared.merge(local).to_shared().subscribe(move |v| {
      res.push(v);
    });
  }
}
