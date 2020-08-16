use crate::prelude::*;
use crate::scheduler::SharedScheduler;
use observable::observable_proxy_impl;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ObserveOnOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(ObserveOnOp, S, SD);

impl<S, SD> LocalObservable<'static> for ObserveOnOp<S, SD>
where
  S: LocalObservable<'static>,
  S::Item: Clone + 'static,
  S::Err: Clone + 'static,
  SD: LocalScheduler + 'static,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'static>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let observer = ObserveOnObserver {
      observer: Rc::new(RefCell::new(subscriber.observer)),
      proxy: subscriber.subscription.clone(),
      scheduler: self.scheduler,
    };

    self.source.actual_subscribe(Subscriber {
      observer,
      subscription: subscriber.subscription,
    })
  }
}

impl<S, SD> SharedObservable for ObserveOnOp<S, SD>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
  S::Err: Clone + Send + 'static,
  SD: SharedScheduler + Send + Sync + 'static,
{
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
  observer: O,
  proxy: U,
  scheduler: SD,
}

#[doc(hidden)]
macro impl_observer($item: ident, $err: ident, $($path: ident).*) {
  fn next(&mut self, value: $item) {
    let s = self.scheduler.schedule(
      |subscription, state| {
        if !subscription.is_closed() {
          let (v, observer) = state;
          observer.$($path()).*.next(v);
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
          observer.$($path()).*.error(e);
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
          observer.$($path()).*.complete();
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
  for ObserveOnObserver<Arc<Mutex<O>>, SD, SharedSubscription>
where
  Item: Clone + Send + 'static,
  Err: Clone + Send + 'static,
  O: Observer<Item, Err> + Send + 'static,
  SD: SharedScheduler,
{
  impl_observer!(Item, Err, lock.unwrap);
}

impl<Item, Err, O, SD> Observer<Item, Err>
  for ObserveOnObserver<Rc<RefCell<O>>, SD, LocalSubscription>
where
  Item: Clone + 'static,
  Err: Clone + 'static,
  O: Observer<Item, Err> + 'static,
  SD: LocalScheduler,
{
  impl_observer!(Item, Err, borrow_mut);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::ThreadPool;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;

  #[test]
  fn switch_thread() {
    let id = thread::spawn(move || {}).thread().id();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(vec![]));
    let oc = observe_thread.clone();

    let pool = ThreadPool::builder().pool_size(10).create().unwrap();

    observable::create(|mut s| {
      s.next(&1);
      s.next(&1);
      *emit_thread.lock().unwrap() = thread::current().id();
    })
    .observe_on(pool.clone())
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
  fn pool_unsubscribe() {
    let scheduler = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .to_shared()
      .observe_on(scheduler.clone())
      .delay(Duration::from_millis(10), scheduler)
      .to_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
