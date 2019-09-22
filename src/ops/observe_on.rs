use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::sync::{Arc, Mutex};

/// Re-emits all notifications from source Observable with specified scheduler.
///
/// `ObserveOn` is an operator that accepts a scheduler as the parameter,
/// which will be used to reschedule notifications emitted by the source
/// Observable.
pub trait ObserveOn {
  fn observe_on<SD>(self, scheduler: SD) -> ObserveOnOp<Self, SD>
  where
    Self: Sized,
  {
    ObserveOnOp {
      source: self,
      scheduler,
    }
  }
}

pub struct ObserveOnOp<S, SD> {
  source: S,
  scheduler: SD,
}

impl<S> ObserveOn for S {}

impl<S, SD> IntoShared for ObserveOnOp<S, SD>
where
  S: Send + Sync + 'static,
  SD: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}

impl<S, Item, Err, Sub, SD> RawSubscribable<Item, Err, Sub>
  for ObserveOnOp<S, SD>
where
  Sub: Subscribe<Item, Err> + IntoShared,
  S: RawSubscribable<Item, Err, ObserveOnSubscribe<Sub::Shared, SD>>,
  S::Unsub: IntoShared,
  <S::Unsub as IntoShared>::Shared: SubscriptionLike,
{
  type Unsub = SharedSubscription;
  fn raw_subscribe(self, subscribe: Sub) -> Self::Unsub {
    let mut subscription = SharedSubscription::default();
    let observe_subscribe = ObserveOnSubscribe {
      subscribe: Arc::new(Mutex::new(subscribe.to_shared())),
      proxy: subscription.clone(),
      scheduler: self.scheduler,
    };
    let s = self.source.raw_subscribe(observe_subscribe);
    subscription.add(Box::new(s.to_shared()));
    subscription
  }
}

pub struct ObserveOnSubscribe<Sub, SD> {
  subscribe: Arc<Mutex<Sub>>,
  proxy: SharedSubscription,
  scheduler: SD,
}

impl<Sub, SD> IntoShared for ObserveOnSubscribe<Sub, SD>
where
  Self: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

impl<Item, Err, Sub, SD> Subscribe<Item, Err> for ObserveOnSubscribe<Sub, SD>
where
  Sub: Subscribe<Item, Err> + Send + Sync + 'static,
  Item: Clone + Send + Sync + 'static,
  Err: Clone + Send + Sync + 'static,
  SD: Scheduler,
{
  fn on_next(&mut self, value: &Item) {
    let s = self.scheduler.schedule(
      |subscription, state| {
        if !subscription.is_closed() {
          let (v, subscribe) = state;
          subscribe.lock().unwrap().on_next(&v);
        }
      },
      (value.clone(), self.subscribe.clone()),
    );
    self.proxy.add(Box::new(s));
  }
  fn on_error(&mut self, err: &Err) {
    let s = self.scheduler.schedule(
      |mut subscription, state| {
        if !subscription.is_closed() {
          let (e, subscribe) = state;
          subscribe.lock().unwrap().on_error(&e);
          subscription.unsubscribe();
        }
      },
      (err.clone(), self.subscribe.clone()),
    );
    self.proxy.add(Box::new(s));
  }
  fn on_complete(&mut self) {
    let s = self.scheduler.schedule(
      |mut subscription, subscribe| {
        if !subscription.is_closed() {
          subscribe.lock().unwrap().on_complete();
          subscription.unsubscribe();
        }
      },
      self.subscribe.clone(),
    );
    self.proxy.add(Box::new(s));
  }
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
    Observable::new(|mut s| {
      s.next(&1);
      s.next(&1);
      *emit_thread.lock().unwrap() = thread::current().id();
    })
    .observe_on(Schedulers::NewThread)
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
    observable::from_iter!(0..10)
      .observe_on(scheduler)
      .delay(Duration::from_millis(10))
      .subscribe(move |v| {
        emitted.lock().unwrap().push(*v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
