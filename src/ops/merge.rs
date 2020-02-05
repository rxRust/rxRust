use crate::observer::observer_next_proxy_impl;
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
/// let numbers = Subject::local();
/// // crate a even stream by filter
/// let even = numbers.fork().filter(|v| *v % 2 == 0);
/// // crate an odd stream by filter
/// let odd = numbers.fork().filter(|v| *v % 2 != 0);
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

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

pub struct SharedMergeOp<S1, S2>(MergeOp<S1, S2>);

type LocalMergeSubscriber<O> =
  Subscriber<LocalMergeObserver<O>, LocalSubscription>;

type SharedMergeSubscriber<O> =
  Subscriber<SharedMergeObserver<O>, SharedSubscription>;

macro merge_subscribe(
  $op:ident, $subscriber:ident,
  $unsub_type: ty, $observer_creator: ident) {{
  let mut subscription = <$unsub_type>::default();
  let downstream = $subscriber.subscription;
  subscription.add(downstream.clone());
  let merge_observer =
    $observer_creator($subscriber.observer, subscription.clone());
  subscription.add($op.source1.raw_subscribe(Subscriber {
    observer: merge_observer.clone(),
    subscription: <$unsub_type>::default(),
  }));
  subscription.add($op.source2.raw_subscribe(Subscriber {
    observer: merge_observer,
    subscription: <$unsub_type>::default(),
  }));
  subscription
}}

impl<S1, S2, O> RawSubscribable<Subscriber<O, LocalSubscription>>
  for MergeOp<S1, S2>
where
  S1: RawSubscribable<LocalMergeSubscriber<O>>,
  S2: RawSubscribable<LocalMergeSubscriber<O>>,
{
  type Unsub = LocalSubscription;

  fn raw_subscribe(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    merge_subscribe!(self, subscriber, LocalSubscription, local_observer)
  }
}

impl<S1, S2, O, U> RawSubscribable<Subscriber<O, U>> for SharedMergeOp<S1, S2>
where
  S1: RawSubscribable<
    SharedMergeSubscriber<O::Shared>,
    Unsub = SharedSubscription,
  >,
  S2: RawSubscribable<
    SharedMergeSubscriber<O::Shared>,
    Unsub = SharedSubscription,
  >,
  O: IntoShared,
  U: IntoShared<Shared = SharedSubscription>,
{
  type Unsub = SharedSubscription;

  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let subscriber = subscriber.to_shared();
    let merge = self.0;
    merge_subscribe!(merge, subscriber, SharedSubscription, shared_observer)
  }
}

impl<S1, S2> Fork for MergeOp<S1, S2>
where
  S1: Fork,
  S2: Fork,
{
  type Output = MergeOp<S1::Output, S2::Output>;
  fn fork(&self) -> Self::Output {
    MergeOp {
      source1: self.source1.fork(),
      source2: self.source2.fork(),
    }
  }
}

#[derive(Clone)]
pub struct MergeObserver<O, Unsub> {
  observer: O,
  subscription: Unsub,
  completed_one: bool,
}

type LocalMergeObserver<O> = Rc<RefCell<MergeObserver<O, LocalSubscription>>>;
type SharedMergeObserver<O> = Arc<Mutex<MergeObserver<O, SharedSubscription>>>;

fn local_observer<O>(
  observer: O,
  subscription: LocalSubscription,
) -> LocalMergeObserver<O> {
  Rc::new(RefCell::new(MergeObserver {
    observer,
    subscription,
    completed_one: false,
  }))
}

fn shared_observer<O>(
  observer: O,
  subscription: SharedSubscription,
) -> SharedMergeObserver<O> {
  Arc::new(Mutex::new(MergeObserver {
    observer,
    subscription,
    completed_one: false,
  }))
}

observer_next_proxy_impl!(MergeObserver<O, Unsub>, O, observer, <O, Unsub>);

impl<Err, O, Unsub> ObserverError<Err> for MergeObserver<O, Unsub>
where
  O: ObserverError<Err>,
  Unsub: SubscriptionLike,
{
  fn error(&mut self, err: Err) {
    self.observer.error(err);
    self.subscription.unsubscribe();
  }
}

impl<O, Unsub> ObserverComplete for MergeObserver<O, Unsub>
where
  O: ObserverComplete,
  Unsub: SubscriptionLike,
{
  fn complete(&mut self) {
    if self.completed_one {
      self.observer.complete();
      self.subscription.unsubscribe();
    } else {
      self.completed_one = true;
    }
  }
}

impl<O, U> IntoShared for MergeObserver<O, U>
where
  O: IntoShared,
  U: IntoShared,
{
  type Shared = MergeObserver<O::Shared, U::Shared>;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared {
    MergeObserver {
      observer: self.observer.to_shared(),
      subscription: self.subscription.to_shared(),
      completed_one: self.completed_one,
    }
  }
}

impl<S1, S2> IntoShared for MergeOp<S1, S2>
where
  S1: IntoShared,
  S2: IntoShared,
{
  type Shared = SharedMergeOp<S1::Shared, S2::Shared>;
  fn to_shared(self) -> Self::Shared {
    SharedMergeOp(MergeOp {
      source1: self.source1.to_shared(),
      source2: self.source2.to_shared(),
    })
  }
}

impl<S1, S2> IntoShared for SharedMergeOp<S1, S2>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Fork, Merge},
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
      let mut numbers = Subject::local();
      // enabling multiple observers for even stream;
      let even = numbers.fork().filter(|v| *v % 2 == 0);
      // enabling multiple observers for odd stream;
      let odd = numbers.fork().filter(|v| *v % 2 != 0);

      // merge odd and even stream again
      let merged = even.fork().merge(odd.fork());

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
    let mut numbers = Subject::local();
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
    let mut even = Subject::local();
    let mut odd = Subject::local();

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
    let mut even = Subject::local();
    let mut odd = Subject::local();

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

    let m = o.fork().merge(o.fork());
    m.fork().merge(m.fork()).subscribe(|_| {});
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
