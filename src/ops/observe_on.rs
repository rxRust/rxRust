use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::sync::Arc;

/// Re-emits all notifications from source Observable with specified scheduler.
///
/// `ObserveOn` is an operator that accepts a scheduler as the parameter,
/// which will be used to reschedule notifications emitted by the source
/// Observable.
pub trait ObserveOn<Sub> {
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

impl<S, Sub> ObserveOn<Sub> for S
where
  S: RawSubscribable<Sub>,
  Sub: Subscribe,
{
}

impl<S, Sub, SD> RawSubscribable<Sub> for ObserveOnOp<S, SD>
where
  Sub: Subscribe + Send + Sync + 'static,
  S: RawSubscribable<ObserveOnSubscribe<Sub, SD>>,
  Sub::Item: Clone + Send + Sync + 'static,
  Sub::Err: Clone + Send + Sync + 'static,
  SD: Scheduler + Send + Sync + 'static,
  S::Unsub: Subscription + Send + Sync + 'static,
{
  type Unsub = SubscriptionProxy;
  fn raw_subscribe(self, subscribe: Sub) -> Self::Unsub {
    let proxy = SubscriptionProxy::new();
    let observe_subscribe = ObserveOnSubscribe {
      subscribe: Arc::new(subscribe),
      proxy: proxy.clone(),
      scheduler: self.scheduler,
    };
    let subscription = self.source.raw_subscribe(observe_subscribe);
    proxy.proxy(subscription);
    proxy
  }
}

pub struct ObserveOnSubscribe<Sub, SD> {
  subscribe: Arc<Sub>,
  proxy: SubscriptionProxy,
  scheduler: SD,
}

impl<Sub, SD> Subscribe for ObserveOnSubscribe<Sub, SD>
where
  Sub: Subscribe + Send + Sync + 'static,
  Sub::Item: Clone + Send + 'static,
  Sub::Err: Clone + Send + 'static,
  SD: Scheduler,
{
  type Item = Sub::Item;
  type Err = Sub::Err;

  fn run(&self, v: RxValue<&'_ Self::Item, &'_ Self::Err>) {
    let s = self.scheduler.schedule(
      |proxy, value| {
        if !proxy.is_stopped() {
          if let Some((rv, subscribe)) = value {
            subscribe.run(rv.as_ref());
          }
        }
      },
      Some((v.to_owned(), self.subscribe.clone())),
    );
    self.proxy.proxy(s);
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
    Observable::<_, _, ()>::new(|s| {
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

  #[test]
  fn sync_unsubscribe() { unsubscribe_scheduler(Schedulers::Sync) }

  fn unsubscribe_scheduler(scheduler: Schedulers) {
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_range(0..10)
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
