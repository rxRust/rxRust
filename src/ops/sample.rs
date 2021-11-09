use crate::{impl_helper::*, impl_local_shared_both, prelude::*};

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

impl_local_shared_both! {
  impl<Source, Sampling> SampleOp<Source, Sampling>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let subscription =  $ctx::RcMultiSubscription::default();
    let source_observer = $ctx::Rc::own(SampleObserver {
      observer: $observer,
      value: Option::None,
      subscription: subscription.clone(),
      done: false,
    });

    subscription.add($self.sampling.actual_subscribe(SamplingObserver(
      source_observer.clone(),
      TypeHint::new(),
    )));
    subscription.add($self.source.actual_subscribe(source_observer));
    subscription
  }
  where
    @ctx::shared_only(
      Source::Item: Send + Sync + 'static,
      Sampling::Item: 'static,
    )
    @ctx::local_only(Sampling::Item: 'o,)
    Source: @ctx::Observable,
    Sampling: @ctx::Observable<Err=Source::Err>,
    Source::Unsub: 'static,
    Sampling::Unsub: 'static
}

#[derive(Clone)]
struct SampleObserver<O: Observer, Unsub> {
  observer: O,
  value: Option<O::Item>,
  subscription: Unsub,
  done: bool,
}

impl<O, Unsub> Observer for SampleObserver<O, Unsub>
where
  O: Observer,
  Unsub: SubscriptionLike,
{
  type Item = O::Item;
  type Err = O::Err;
  fn next(&mut self, value: Self::Item) { self.value = Some(value); }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) {
    if !self.done {
      self.subscription.unsubscribe();
      self.done = true;
    }
  }
}

trait DrainValue<Item, Err> {
  fn drain_value(&mut self);
}

impl<O, Unsub> DrainValue<O::Item, O::Err> for SampleObserver<O, Unsub>
where
  O: Observer,
{
  fn drain_value(&mut self) {
    if self.done || self.value.is_none() {
      return;
    }
    let value = self.value.take().unwrap();
    self.observer.next(value);
  }
}

impl<O, Unsub> DrainValue<O::Item, O::Err> for MutRc<SampleObserver<O, Unsub>>
where
  O: Observer,
{
  fn drain_value(&mut self) {
    let mut val = self.rc_deref_mut();
    val.drain_value();
  }
}

impl<O, Unsub> DrainValue<O::Item, O::Err> for MutArc<SampleObserver<O, Unsub>>
where
  O: Observer,
{
  fn drain_value(&mut self) {
    let mut val = self.rc_deref_mut();
    val.drain_value();
  }
}

struct SamplingObserver<Item, O>(O, TypeHint<*const Item>);

impl<Item, Item2, Err, O> Observer for SamplingObserver<Item2, O>
where
  O: DrainValue<Item, Err> + Observer<Item = Item, Err = Err>,
{
  type Item = Item2;
  type Err = Err;

  fn next(&mut self, _: Item2) { self.0.drain_value(); }

  fn complete(&mut self) {
    self.0.drain_value();
    self.0.complete();
  }

  fn error(&mut self, err: Self::Err) { self.0.error(err) }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use crate::test_scheduler::ManualScheduler;
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
  };

  #[test]
  fn sample_base() {
    let scheduler = ManualScheduler::now();
    let x = Rc::new(RefCell::new(vec![]));

    let interval =
      observable::interval(Duration::from_millis(1), scheduler.clone());
    {
      let x_c = x.clone();
      interval
        .take(100)
        .sample(observable::interval(
          Duration::from_millis(10),
          scheduler.clone(),
        ))
        .subscribe(move |v| {
          x_c.borrow_mut().push(v);
        });

      scheduler.advance_and_run(Duration::from_millis(1), 100);
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
      .into_shared()
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
