use crate::observer::error_proxy_impl;
use crate::prelude::*;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

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
  Sampling: SharedObservable<Err = Source::Err, Unsub = Source::Unsub>,
  Sampling::Item: Send + Sync + 'static + Clone,
{
  type Unsub = SharedSubscription;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let mut subscription = subscriber.subscription;
    let source_observer = Arc::new(Mutex::new(SampleObserver {
      observer: subscriber.observer,
      value: Option::None,
      subscription: subscription.clone(),
      done: false,
      _marker: PhantomData,
    }));

    subscription.add(self.sampling.actual_subscribe(Subscriber {
      observer: SamplingObserverShared(source_observer.clone()),
      subscription: SharedSubscription::default(),
    }));
    subscription.add(self.source.actual_subscribe(Subscriber {
      observer: source_observer.clone(),
      subscription: SharedSubscription::default(),
    }));
    subscription
  }
}

impl<'a, Source, Sampling> LocalObservable<'a> for SampleOp<Source, Sampling>
where
  Source: LocalObservable<'a> + 'a,
  Sampling: LocalObservable<'a, Err = Source::Err, Unsub = Source::Unsub> + 'a,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let mut subscription = subscriber.subscription;
    let source_observer = Rc::new(RefCell::new(SampleObserver {
      observer: subscriber.observer,
      value: Option::None,
      subscription: subscription.clone(),
      done: false,
      _marker: PhantomData,
    }));

    subscription.add(self.sampling.actual_subscribe(Subscriber {
      observer: SamplingObserverLocal(source_observer.clone()),
      subscription: LocalSubscription::default(),
    }));
    subscription.add(self.source.actual_subscribe(Subscriber {
      observer: source_observer.clone(),
      subscription: LocalSubscription::default(),
    }));
    subscription
  }
}

#[derive(Clone)]
struct SampleObserver<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
{
  observer: O,
  value: Option<Item>,
  subscription: Unsub,
  done: bool,

  _marker: PhantomData<Err>,
}

impl<Item, Err, O, Unsub> Observer<Item, Err>
  for SampleObserver<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
  Unsub: SubscriptionLike,
{
  fn next(&mut self, value: Item) { self.value = Some(value); }

  error_proxy_impl!(Err, observer);

  fn complete(&mut self) { self.subscription.unsubscribe(); }
}

pub struct SamplingObserverShared<Item, Err, O, Unsub>(
  Arc<Mutex<SampleObserver<Item, Err, O, Unsub>>>,
)
where
  O: Observer<Item, Err>;

impl<Item, Err, O, Unsub> SamplingObserverShared<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
{
  fn emit(&mut self) {
    let mut val = self.0.lock().unwrap();
    if val.done || val.value.is_none() {
      return;
    }
    let value = val.value.take();
    val.observer.next(value.unwrap());
  }
}

impl<Item, Item2, Err, O, Unsub> Observer<Item2, Err>
  for SamplingObserverShared<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
  Unsub: SubscriptionLike,
  Item: Clone,
{
  fn next(&mut self, _: Item2) { self.emit(); }

  fn error(&mut self, err: Err) {
    let mut val = self.0.lock().unwrap();
    val.observer.error(err);
  }

  fn complete(&mut self) {
    self.emit();
    let mut val = self.0.lock().unwrap();
    if !val.done {
      val.subscription.unsubscribe();
    }
    val.done = true;
  }
}

pub struct SamplingObserverLocal<Item, Err, O, Unsub>(
  Rc<RefCell<SampleObserver<Item, Err, O, Unsub>>>,
)
where
  O: Observer<Item, Err>;

impl<Item, Err, O, Unsub> SamplingObserverLocal<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
{
  fn emit(&mut self) {
    let mut val = self.0.borrow_mut();
    if val.done || val.value.is_none() {
      return;
    }
    let value = val.value.take();
    val.observer.next(value.unwrap());
  }
}

impl<Item, Item2, Err, O, Unsub> Observer<Item2, Err>
  for SamplingObserverLocal<Item, Err, O, Unsub>
where
  O: Observer<Item, Err>,
  Unsub: SubscriptionLike,
{
  fn next(&mut self, _: Item2) { self.emit(); }

  fn error(&mut self, err: Err) {
    let mut val = self.0.borrow_mut();
    val.observer.error(err);
  }

  fn complete(&mut self) {
    self.emit();
    let mut val = self.0.borrow_mut();
    if !val.done {
      val.subscription.unsubscribe();
    }
    val.done = true;
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::{Arc, Mutex};
  #[test]
  fn sample_base() {
    let x = Arc::new(Mutex::new(vec![]));

    let interval = observable::interval(std::time::Duration::from_millis(5));
    {
      let x_c = x.clone();
      let mut sub = interval
        .sample(observable::interval(std::time::Duration::from_millis(11)))
        .to_shared()
        .subscribe(move |v| {
          let mut l = x_c.lock().unwrap();
          l.push(v);
        });
      std::thread::sleep(std::time::Duration::from_millis(115));
      sub.unsubscribe();
      {
        let v = x.lock().unwrap().clone();
        assert_eq!(v, vec![1, 3, 5, 7, 10, 12, 14, 16, 18, 21]);
      }
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
