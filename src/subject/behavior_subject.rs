use crate::prelude::*;

#[derive(Clone)]
pub struct BehaviorSubject<S, V> {
  pub(crate) subject: S,
  pub(crate) value: V,
}

pub type LocalBehaviorSubject<'a, Item, Err> =
  BehaviorSubject<LocalSubject<'a, Item, Err>, MutRc<Item>>;

pub type SharedBehaviorSubject<Item, Err> =
  BehaviorSubject<SharedSubject<Item, Err>, MutArc<Item>>;

impl<'a, Item, Err> LocalBehaviorSubject<'a, Item, Err> {
  #[inline]
  pub fn new(value: Item) -> Self {
    Self {
      subject: <_>::default(),
      value: MutRc::own(value),
    }
  }
}
impl<Item, Err> SharedBehaviorSubject<Item, Err> {
  #[inline]
  pub fn new(value: Item) -> Self {
    Self {
      subject: <_>::default(),
      value: MutArc::own(value),
    }
  }
}

impl<S: SubscriptionLike, V> SubscriptionLike for BehaviorSubject<S, V> {
  #[inline]
  fn unsubscribe(&mut self) { self.subject.unsubscribe(); }

  #[inline]
  fn is_closed(&self) -> bool { self.subject.is_closed() }
}

impl<S, Item> Observer for BehaviorSubject<S, Item>
where
  S: Observer,
  S::Item: Clone,
  Item: RcDerefMut,
  for<'r> Item::Target<'r>: std::ops::DerefMut<Target = S::Item>,
{
  type Item = S::Item;
  type Err = S::Err;

  #[inline]
  fn next(&mut self, value: Self::Item) {
    let mut v = self.value.rc_deref_mut();
    *v = value;
    self.subject.next(v.clone())
  }

  #[inline]
  fn error(&mut self, err: Self::Err) { self.subject.error(err) }

  #[inline]
  fn complete(&mut self) { self.subject.complete() }
}

impl<S: TearDownSize, V> TearDownSize for BehaviorSubject<S, V> {
  #[inline]
  fn teardown_size(&self) -> usize { self.subject.teardown_size() }
}

impl<Item, Err> Observable for SharedBehaviorSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedBehaviorSubject<Item, Err>
where
  Item: Clone,
  Err: Clone,
{
  type Unsub = MutArc<SingleSubscription>;
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    if !self.subject.is_closed() {
      observer.next(self.value.rc_deref().clone());
    }
    let o = Box::new(observer);
    self.subject.actual_subscribe(o)
  }
}

impl<'a, Item, Err> Observable for LocalBehaviorSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item: Clone, Err> LocalObservable<'a>
  for LocalBehaviorSubject<'a, Item, Err>
{
  type Unsub = MutRc<SingleSubscription>;
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    if !self.subject.is_closed() {
      observer.next(self.value.rc_deref().clone());
    }
    self.subject.actual_subscribe(observer)
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  #[cfg(not(target_arch = "wasm32"))]
  use std::time::{Duration, Instant};

  #[test]
  fn base_data_flow() {
    let mut i = 0;

    {
      let broadcast = LocalBehaviorSubject::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
    }

    assert_eq!(i, 84);

    {
      let mut broadcast = LocalBehaviorSubject::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }

    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = LocalBehaviorSubject::new(42);
    broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!("{}", e));

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let subject = LocalBehaviorSubject::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
    }

    assert_eq!(i, 42);

    {
      let mut subject = LocalBehaviorSubject::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 42);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn empty_local_subject_can_convert_into_shared() {
    let pool = ThreadPool::new().unwrap();
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let subject = SharedBehaviorSubject::new(42);
    let mut subject_c = subject.clone();
    let stamp = Instant::now();
    pool.schedule(
      move |_| {
        subject_c.complete();
      },
      Some(Duration::from_millis(25)),
      (),
    );
    subject
      .clone()
      .into_shared()
      .observe_on(pool)
      .into_shared()
      .subscribe_blocking(move |v: i32| {
        *value.lock().unwrap() = v;
      });
    assert!(stamp.elapsed() > Duration::from_millis(25));
    assert_eq!(*c_v.lock().unwrap(), 42);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = LocalBehaviorSubject::new(42);
    let local2 = LocalBehaviorSubject::new(42);
    local.clone().actual_subscribe(local2);
    local.next(1);
    local.error(2);
  }
}
