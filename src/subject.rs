use crate::prelude::*;
use std::ops::DerefMut;

pub mod behavior_subject;
pub use behavior_subject::*;

pub enum ObserverTrigger<Item, Err> {
  Item(Item),
  Err(Err),
  Complete,
}
pub struct InnerSubject<O: Observer + ?Sized, S: SubscriptionLike> {
  observers: Vec<SubjectObserver<Box<O>, S>>,
  subscription: SingleSubscription,
}

#[derive(Default, Clone)]
pub struct Subject<T, B> {
  inner: T,
  chamber: T,
  buffer: B,
}

pub type SharedSubject<Item, Err> = Subject<
  MutArc<
    InnerSubject<
      dyn Observer<Item = Item, Err = Err> + Send + Sync,
      MutArc<SingleSubscription>,
    >,
  >,
  MutArc<Vec<ObserverTrigger<Item, Err>>>,
>;

pub type LocalSubject<'a, Item, Err> = Subject<
  MutRc<
    InnerSubject<
      dyn Observer<Item = Item, Err = Err> + 'a,
      MutRc<SingleSubscription>,
    >,
  >,
  MutRc<Vec<ObserverTrigger<Item, Err>>>,
>;

impl<Item, Err> SharedSubject<Item, Err> {
  #[inline]
  pub fn new() -> Self { Self::default() }
}

impl<'a, Item, Err> LocalSubject<'a, Item, Err> {
  #[inline]
  pub fn new() -> Self { Self::default() }
}

fn emit_buffer<'a, O, B, Item, Err, T>(mut observer: O, b: &'a B)
where
  O: DerefMut,
  O::Target: Observer<Item = Item, Err = Err>,
  B: RcDerefMut<Target<'a> = T> + 'a,
  T: DerefMut<Target = Vec<ObserverTrigger<Item, Err>>>,
{
  loop {
    let v = b.rc_deref_mut().pop();
    if let Some(to_emit) = v {
      match to_emit {
        ObserverTrigger::Item(v) => observer.next(v),
        ObserverTrigger::Err(err) => observer.error(err),
        ObserverTrigger::Complete => observer.complete(),
      }
    } else {
      break;
    }
  }
}

macro_rules! impl_observer {
  () => {
    type Item = Item;
    type Err = Err;

    fn next(&mut self, value: Self::Item) {
      if let Ok(mut inner) = self.inner.try_rc_deref_mut() {
        inner.load(self.chamber.rc_deref_mut().unload());
        inner.next(value);
        emit_buffer(inner, &self.buffer)
      } else {
        self
          .buffer
          .rc_deref_mut()
          .push(ObserverTrigger::Item(value));
      }
    }

    fn error(&mut self, err: Self::Err) {
      if let Ok(mut inner) = self.inner.try_rc_deref_mut() {
        inner.load(self.chamber.rc_deref_mut().unload());
        inner.error(err);
        emit_buffer(inner, &self.buffer)
      } else {
        self.buffer.rc_deref_mut().push(ObserverTrigger::Err(err));
      }
    }

    fn complete(&mut self) {
      if let Ok(mut inner) = self.inner.try_rc_deref_mut() {
        inner.load(self.chamber.rc_deref_mut().unload());
        inner.complete();
        emit_buffer(inner, &self.buffer)
      } else {
        self.buffer.rc_deref_mut().push(ObserverTrigger::Complete);
      }
    }
  };
}

impl<'a, Item: Clone, Err: Clone> Observer for LocalSubject<'a, Item, Err> {
  impl_observer!();
}

impl<Item: Clone, Err: Clone> Observer for SharedSubject<Item, Err> {
  impl_observer!();
}

impl<Item, Err> Observable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Unsub = MutArc<SingleSubscription>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self.chamber.rc_deref_mut().subscribe(Box::new(observer))
  }
}

impl<'a, Item, Err> Observable for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalObservable<'a> for LocalSubject<'a, Item, Err> {
  type Unsub = MutRc<SingleSubscription>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.chamber.rc_deref_mut().subscribe(Box::new(observer))
  }
}

impl<O, U> SubscriptionLike for InnerSubject<O, U>
where
  O: Observer + ?Sized,
  U: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self
      .observers
      .iter_mut()
      .for_each(|o| o.subscription.unsubscribe());
    self.observers.clear();
    self.subscription.unsubscribe();
  }

  #[inline]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

impl<T: SubscriptionLike, B> SubscriptionLike for Subject<T, B> {
  #[inline]
  fn unsubscribe(&mut self) { self.inner.unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.inner.is_closed() }
}

impl<O, U> TearDownSize for InnerSubject<O, U>
where
  O: Observer + ?Sized,
  U: SubscriptionLike,
{
  #[inline]
  fn teardown_size(&self) -> usize { self.observers.len() }
}

impl<T: TearDownSize, B> TearDownSize for Subject<T, B> {
  #[inline]
  fn teardown_size(&self) -> usize {
    self.inner.teardown_size() + self.chamber.teardown_size()
  }
}

pub(crate) struct SubjectObserver<O, S> {
  pub observer: O,
  pub subscription: S,
}

impl<O: Observer + ?Sized, S: SubscriptionLike> Default for InnerSubject<O, S> {
  fn default() -> Self {
    InnerSubject {
      observers: vec![],
      subscription: Default::default(),
    }
  }
}

impl<O, U> Observer for InnerSubject<O, U>
where
  O: Observer + ?Sized,
  U: SubscriptionLike,
  O::Item: Clone,
  O::Err: Clone,
{
  type Item = O::Item;
  type Err = O::Err;

  fn next(&mut self, value: Self::Item) {
    if !self.subscription.is_closed() {
      let any_finished =
        self.observers.iter_mut().fold(false, |finished, o| {
          if !o.subscription.is_closed() {
            o.observer.next(value.clone());
          }
          finished || o.subscription.is_closed()
        });

      if any_finished {
        self.observers.retain(|o| !o.subscription.is_closed());
      }
    } else {
      self.observers.clear();
    }
  }

  fn error(&mut self, err: Self::Err) {
    if !self.is_closed() {
      self
        .observers
        .iter_mut()
        .for_each(|subscriber| subscriber.observer.error(err.clone()));
      self.observers.clear();
    }
  }

  fn complete(&mut self) {
    if !self.is_closed() {
      self
        .observers
        .iter_mut()
        .for_each(|subscriber| subscriber.observer.complete());
      self.observers.clear();
    }
  }
}

impl<O, U> InnerSubject<O, U>
where
  O: Observer + ?Sized,
  U: SubscriptionLike + Default + Clone,
{
  fn subscribe(&mut self, observer: Box<O>) -> U {
    let subscription = U::default();
    let observer = SubjectObserver {
      observer,
      subscription: subscription.clone(),
    };
    self.observers.push(observer);
    subscription
  }

  fn load(&mut self, mut observers: Vec<SubjectObserver<Box<O>, U>>) {
    self.observers.append(&mut observers);
  }

  #[inline]
  fn unload(&mut self) -> Vec<SubjectObserver<Box<O>, U>> {
    std::mem::take(&mut self.observers)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use futures::executor::ThreadPool;
  use std::time::{Duration, Instant};

  #[test]
  fn smoke() {
    let test_code = MutArc::own("".to_owned());
    let mut subject = SharedSubject::new();
    let c_test_code = test_code.clone();
    subject.clone().into_shared().subscribe(move |v: &str| {
      *c_test_code.rc_deref_mut() = v.to_owned();
    });
    subject.next("test shared subject");
    assert_eq!(*test_code.rc_deref_mut(), "test shared subject");
    assert_eq!(subject.teardown_size(), 1);
  }

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = LocalSubject::default();
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = LocalSubject::new();
    broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!("{}", e));
    broadcast.next(1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = LocalSubject::new();
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn empty_local_subject_can_convert_into_shared() {
    let pool = ThreadPool::new().unwrap();
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let subject = SharedSubject::new();
    let mut subject_c = subject.clone();
    let stamp = Instant::now();
    pool.schedule(
      move |_| {
        subject_c.next(100);
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
    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = LocalSubject::new();
    let local2 = LocalSubject::new();
    local.clone().actual_subscribe(local2);
    local.next(1);
    local.error(2);
  }

  #[test]
  fn fix_recursive_next() {
    let mut subject = Subject::default();
    let mut c_subject = subject.clone();

    subject.clone().subscribe(move |i| {
      if i < 2 {
        c_subject.next(i + 1)
      }
    });

    subject.next(0);

    let mut subject = Subject::default();
    let mut c_subject = subject.clone();

    subject.clone().into_shared().subscribe(move |i| {
      if i < 2 {
        c_subject.next(i + 1)
      }
    });

    subject.next(0);
  }
}
