use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

/// Re-emits all notifications from source Observable with specified scheduler.
///
/// `ObserveOn` is an operator that accepts a scheduler as the parameter,
/// which will be used to reschedule notifications emitted by the source
/// Observable.
pub trait ObserveOn<'a> {
  fn observe_on<SD>(self, scheduler: SD) -> ObserveOnOp<'a, Self, SD>
  where
    Self: Sized,
  {
    ObserveOnOp {
      source: self,
      scheduler,
      _p: PhantomData,
    }
  }
}

pub struct ObserveOnOp<'a, S, SD> {
  source: S,
  scheduler: SD,
  _p: PhantomData<&'a ()>,
}

impl<'a, S> ObserveOn<'a> for S {}

impl<'a, Item, Err, S, SD> SharedObservable for ObserveOnOp<'a, S, SD>
where
  S: Observable<'a, Item = Item, Err = Err>,
  Item: Clone + Send + Sync + 'static,
  Err: Clone + Send + Sync + 'static,
  SD: Scheduler + 'static,
{
  type Item = Item;
  type Err = Err;
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let observer = ObserveOnObserver {
      observer: Arc::new(Mutex::new(subscriber.observer)),
      proxy: subscriber.subscription.clone(),
      scheduler: self.scheduler,
    };

    let mut subscription = LocalSubscription::default();
    subscription.add(subscriber.subscription);
    self.source.actual_subscribe(Subscriber {
      observer,
      subscription,
    })
  }
}

// Fix me. For now, rust generic specialization is not full finished. we can't
// impl two SharedObservable for ObserveOnOp<'a, S, SD>, so we must wrap `S`
// with Shared. And this mean's if any ObserveOnOp's upstream just support
// shared, subscribe, user must call `to_shared` before `observe_on`.
impl<'a, S, SD> SharedObservable for ObserveOnOp<'a, Shared<S>, SD>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
  S::Err: Clone + Send + 'static,
  SD: Scheduler + Send + Sync + 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let subscription = subscriber.subscription;
    let observer = ObserveOnObserver {
      observer: Arc::new(Mutex::new(subscriber.observer)),
      proxy: subscription.clone(),
      scheduler: self.scheduler,
    };
    self.source.actual_subscribe(Subscriber {
      observer,
      subscription,
    })
  }
}

pub struct ObserveOnObserver<O, SD, U> {
  observer: Arc<Mutex<O>>,
  proxy: U,
  scheduler: SD,
}

macro impl_observer($item: ident, $err: ident) {
  fn next(&mut self, value: $item) {
    let s = self.scheduler.schedule(
      |subscription, state| {
        if !subscription.is_closed() {
          let (v, observer) = state;
          observer.lock().unwrap().next(v);
        }
      },
      None,
      (value, self.observer.clone()),
    );
    self.proxy.add(s);
  }

  fn error(&mut self, err: $err) {
    let s = self.scheduler.schedule(
      |mut subscription, state| {
        if !subscription.is_closed() {
          let (e, observer) = state;
          observer.lock().unwrap().error(e);
          subscription.unsubscribe();
        }
      },
      None,
      (err, self.observer.clone()),
    );
    self.proxy.add(s);
  }

  fn complete(&mut self) {
    let s = self.scheduler.schedule(
      |mut subscription, observer| {
        if !subscription.is_closed() {
          observer.lock().unwrap().complete();
          subscription.unsubscribe();
        }
      },
      None,
      self.observer.clone(),
    );
    self.proxy.add(s);
  }
}
impl<Item, Err, O, SD> Observer<Item, Err>
  for ObserveOnObserver<O, SD, SharedSubscription>
where
  Item: Clone + Send + 'static,
  Err: Clone + Send + 'static,
  O: Observer<Item, Err> + Send + 'static,
  SD: Scheduler,
{
  impl_observer!(Item, Err);
}

impl<Item, Err, O, SD> Observer<Item, Err>
  for ObserveOnObserver<O, SD, LocalSubscription>
where
  Item: Clone + Send + 'static,
  Err: Clone + Send + 'static,
  O: Observer<Item, Err> + Send + 'static,
  SD: Scheduler,
{
  impl_observer!(Item, Err);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use crate::{
    ops::{Delay, ObserveOn},
    scheduler::Schedulers,
  };
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;

  #[test]
  fn switch_thread() {
    let id = thread::spawn(move || {}).thread().id();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(vec![]));
    let oc = observe_thread.clone();

    observable::create(|mut s| {
      s.next(&1);
      s.next(&1);
      *emit_thread.lock().unwrap() = thread::current().id();
    })
    .observe_on(Schedulers::NewThread)
    .to_shared()
    .subscribe(move |_v| {
      observe_thread.lock().unwrap().push(thread::current().id());
    });
    std::thread::sleep(Duration::from_millis(1));

    let current_id = thread::current().id();
    assert_eq!(*emit_thread.lock().unwrap(), current_id);
    let ot = oc.lock().unwrap();
    let ot1 = ot[0];
    let ot2 = ot[1];
    assert_ne!(ot1, ot2);
    assert_ne!(current_id, ot2);
    assert_ne!(current_id, ot1);
  }

  #[test]
  fn pool_unsubscribe() { unsubscribe_scheduler(Schedulers::ThreadPool) }

  #[test]
  fn new_thread_unsubscribe() { unsubscribe_scheduler(Schedulers::NewThread) }

  // #[test]
  // fn sync_unsubscribe() { unsubscribe_scheduler(Schedulers::Sync) }

  fn unsubscribe_scheduler(scheduler: Schedulers) {
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .to_shared()
      .observe_on(scheduler)
      .delay(Duration::from_millis(10))
      .to_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
