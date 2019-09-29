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

impl<O> Merge for O {}

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

macro merge_subscribe($op:ident, $subscriber:ident, $unsub_type: ty) {{
  let mut subscription = $subscriber.subscription;
  let merge_subscribe = Subscriber {
    subscribe: MergeSubscribe::<_, $unsub_type>::new(
      $subscriber.subscribe,
      subscription.clone(),
    ),
    subscription: subscription.clone(),
  };
  subscription.add($op.source1.raw_subscribe(merge_subscribe.clone()));
  subscription.add($op.source2.raw_subscribe(merge_subscribe.clone()));
  subscription
}}

impl<S1, S2, Item, Err, Sub>
  RawSubscribable<Item, Err, Subscriber<Sub, LocalSubscription>>
  for MergeOp<S1, S2>
where
  S1: RawSubscribable<
    Item,
    Err,
    Subscriber<
      MergeSubscribe<Rc<RefCell<Sub>>, LocalSubscription>,
      LocalSubscription,
    >,
  >,
  S2: RawSubscribable<
    Item,
    Err,
    Subscriber<
      MergeSubscribe<Rc<RefCell<Sub>>, LocalSubscription>,
      LocalSubscription,
    >,
  >,
{
  type Unsub = LocalSubscription;

  fn raw_subscribe(
    self,
    subscriber: Subscriber<Sub, LocalSubscription>,
  ) -> Self::Unsub {
    merge_subscribe!(self, subscriber, LocalSubscription)
  }
}

impl<S1, S2, Item, Err, Sub>
  RawSubscribable<Item, Err, Subscriber<Sub, SharedSubscription>>
  for MergeOp<S1, S2>
where
  S1: RawSubscribable<
    Item,
    Err,
    Subscriber<
      MergeSubscribe<Arc<Mutex<Sub>>, SharedSubscription>,
      SharedSubscription,
    >,
    Unsub = SharedSubscription,
  >,
  S2: RawSubscribable<
    Item,
    Err,
    Subscriber<
      MergeSubscribe<Arc<Mutex<Sub>>, SharedSubscription>,
      SharedSubscription,
    >,
    Unsub = SharedSubscription,
  >,
{
  type Unsub = SharedSubscription;

  fn raw_subscribe(
    self,
    subscriber: Subscriber<Sub, SharedSubscription>,
  ) -> Self::Unsub {
    merge_subscribe!(self, subscriber, SharedSubscription)
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
pub struct MergeSubscribe<Sub, Unsub> {
  subscribe: Sub,
  subscription: Unsub,
  completed_one: bool,
}

impl<Sub> MergeSubscribe<Rc<RefCell<Sub>>, LocalSubscription> {
  fn new(subscribe: Sub, subscription: LocalSubscription) -> Self {
    MergeSubscribe {
      subscribe: Rc::new(RefCell::new(subscribe)),
      subscription,
      completed_one: false,
    }
  }
}

impl<Sub> MergeSubscribe<Arc<Mutex<Sub>>, SharedSubscription> {
  fn new(subscribe: Sub, subscription: SharedSubscription) -> Self {
    MergeSubscribe {
      subscribe: Arc::new(Mutex::new(subscribe)),
      subscription,
      completed_one: false,
    }
  }
}

impl<Item, Err, Sub, Unsub> Subscribe<Item, Err> for MergeSubscribe<Sub, Unsub>
where
  Sub: Subscribe<Item, Err>,
  Unsub: SubscriptionLike,
{
  #[inline(always)]
  fn on_next(&mut self, value: &Item) { self.subscribe.on_next(value); }

  fn on_error(&mut self, err: &Err) {
    self.subscribe.on_error(err);
    self.subscription.unsubscribe();
  }

  fn on_complete(&mut self) {
    if self.completed_one {
      self.subscribe.on_complete();
      self.subscription.unsubscribe();
    } else {
      self.completed_one = true;
    }
  }
}

impl<Sub, Unsub> IntoShared for MergeSubscribe<Sub, Unsub>
where
  Self: Sync + Send + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

impl<S1, S2> IntoShared for MergeOp<S1, S2>
where
  S1: IntoShared,
  S2: IntoShared,
{
  type Shared = MergeOp<S1::Shared, S2::Shared>;
  fn to_shared(self) -> Self::Shared {
    MergeOp {
      source1: self.source1.to_shared(),
      source2: self.source2.to_shared(),
    }
  }
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

    even.clone().merge(odd.clone()).subscribe_all(
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
