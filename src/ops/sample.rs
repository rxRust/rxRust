use crate::observer::{error_proxy_impl, is_stopped_proxy_impl};
use crate::prelude::*;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SampleOp<S, N> {
  pub(crate) source: S,
  pub(crate) sampling: N,
}

impl<Source, Sampling> Observable for SampleOp<Source, Sampling>
where
  Source: Observable,
  Sampling: Observable,
{
  type Item = Source::Item;
  type Err = Source::Err;
}

impl<Source, Sampling> SharedObservable for SampleOp<Source, Sampling>
where
  Source: SharedObservable,
  Source::Item: Send + Sync + 'static + Clone,
  Source::Unsub: Send + Sync,
  Source::Err: Send + Sync + 'static,
  Sampling: SharedObservable<Err = Source::Err>,
  Sampling::Item: Send + Sync + 'static + Clone,
  Sampling::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription;
    let source_observer = Arc::new(Mutex::new(SampleObserver {
      observer: subscriber.observer,
      value: Option::None,
      subscription: subscription.clone(),
      done: false,
    }));

    subscription.add(self.sampling.actual_subscribe(Subscriber {
      observer: SamplingObserver(source_observer.clone(), PhantomData),
      subscription: SharedSubscription::default(),
    }));
    subscription.add(self.source.actual_subscribe(Subscriber {
      observer: source_observer,
      subscription: SharedSubscription::default(),
    }));
    subscription
  }
}

impl<'a, Source, Sampling> LocalObservable<'a> for SampleOp<Source, Sampling>
where
  Source: LocalObservable<'a> + 'a,
  Sampling: LocalObservable<'a, Err = Source::Err> + 'a,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription;
    let source_observer = Rc::new(RefCell::new(SampleObserver {
      observer: subscriber.observer,
      value: Option::None,
      subscription: subscription.clone(),
      done: false,
    }));

    subscription.add(self.sampling.actual_subscribe(Subscriber {
      observer: SamplingObserver(source_observer.clone(), PhantomData),
      subscription: LocalSubscription::default(),
    }));
    subscription.add(self.source.actual_subscribe(Subscriber {
      observer: source_observer,
      subscription: LocalSubscription::default(),
    }));
    subscription
  }
}

#[derive(Clone)]
struct SampleObserver<Item, O, Unsub> {
  observer: O,
  value: Option<Item>,
  subscription: Unsub,
  done: bool,
}

impl<Item, O, Unsub> SampleObserver<Item, O, Unsub> {
  fn drain_value<Err>(&mut self)
  where
    O: Observer<Item, Err>,
  {
    if self.done || self.value.is_none() {
      return;
    }
    let value = self.value.take().unwrap();
    self.observer.next(value);
  }
}

impl<Item, Err, O, Unsub> Observer<Item, Err> for SampleObserver<Item, O, Unsub>
where
  O: Observer<Item, Err>,
  Unsub: SubscriptionLike,
{
  fn next(&mut self, value: Item) { self.value = Some(value); }

  error_proxy_impl!(Err, observer);

  fn complete(&mut self) {
    if !self.done {
      self.subscription.unsubscribe();
      self.done = true;
    }
  }
  is_stopped_proxy_impl!(observer);
}

trait DrainValue<Item, Err> {
  fn drain_value(&mut self);
}

impl<Item, Err, O, Unsub> DrainValue<Item, Err>
  for Rc<RefCell<SampleObserver<Item, O, Unsub>>>
where
  O: Observer<Item, Err>,
{
  fn drain_value(&mut self) {
    let mut val = self.borrow_mut();
    val.drain_value();
  }
}

impl<Item, Err, O, Unsub> DrainValue<Item, Err>
  for Arc<Mutex<SampleObserver<Item, O, Unsub>>>
where
  O: Observer<Item, Err>,
{
  fn drain_value(&mut self) {
    let mut val = self.lock().unwrap();
    val.drain_value();
  }
}
struct SamplingObserver<Item, O>(O, PhantomData<Item>);

impl<Item, Item2, Err, O> Observer<Item2, Err> for SamplingObserver<Item, O>
where
  O: DrainValue<Item, Err> + Observer<Item, Err>,
{
  fn next(&mut self, _: Item2) { self.0.drain_value(); }

  error_proxy_impl!(Err, 0);

  fn complete(&mut self) {
    self.0.drain_value();
    self.0.complete();
  }

  is_stopped_proxy_impl!(0);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::LocalPool;
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
  };

  #[test]
  fn sample_base() {
    let mut pool = LocalPool::new();
    let x = Rc::new(RefCell::new(vec![]));

    let interval =
      observable::interval(Duration::from_millis(2), pool.spawner());
    {
      let x_c = x.clone();
      interval
        .take(46)
        .sample(observable::interval(
          Duration::from_millis(10),
          pool.spawner(),
        ))
        .subscribe(move |v| {
          x_c.borrow_mut().push(v);
        });

      pool.run();
      assert_eq!(x.borrow().len(), 10);
    };
  }
  #[test]
  fn sample_by_subject() {
    let mut subject = SharedSubject::new();
    let mut notifier = SharedSubject::new();
    let test_code = Arc::new(Mutex::new(0));
    let c_test_code = test_code.clone();
    subject
      .clone()
      .sample(notifier.clone())
      .to_shared()
      .subscribe(move |v: i32| {
        *c_test_code.lock().unwrap() = v;
      });
    subject.next(1);
    notifier.next(1);
    assert_eq!(*test_code.lock().unwrap(), 1);

    subject.next(2);
    assert_eq!(*test_code.lock().unwrap(), 1);

    subject.next(3);
    notifier.next(1);
    assert_eq!(*test_code.lock().unwrap(), 3);

    *test_code.lock().unwrap() = 0;
    notifier.next(1);
    assert_eq!(*test_code.lock().unwrap(), 0);

    subject.next(4);
    notifier.complete();
    assert_eq!(*test_code.lock().unwrap(), 4);
  }
}
