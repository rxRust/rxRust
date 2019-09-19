use crate::prelude::*;
use crate::scheduler::Scheduler;

/// Specify the Scheduler on which an Observable will operate
///
/// With `SubscribeON` you can decide what type of scheduler a specific
/// Observable will be using when it is subscribed to.
///
/// Schedulers control the speed and order of emissions to observers from an
/// Observable stream.
///
/// # Example
/// Given the following code:
/// ```rust
/// use rxrust::prelude::*;
/// use rxrust::ops::{ Merge };
///
/// let a = observable::from_range(1..5);
/// let b = observable::from_range(5..10);
/// a.merge(b).subscribe(|v| print!("{} ", *v));
/// ```
///
/// Both Observable `a` and `b` will emit their values directly and
/// synchronously once they are subscribed to.
/// This will result in the output of `1 2 3 4 5 6 7 8 9`.
///
/// But if we instead use the `subscribe_on` operator declaring that we want to
/// use the new thread scheduler for values emitted by Observable `a`:
/// ```rust
/// use rxrust::prelude::*;
/// use rxrust::scheduler::Schedulers;
/// use rxrust::ops::{ Merge, SubscribeOn };
/// use std::thread;
///
/// let a = observable::from_range(1..5).subscribe_on(Schedulers::NewThread);
/// let b = observable::from_range(5..10);
/// a.merge(b).subscribe(|v|{
///   let handle = thread::current();
///   print!("{}({:?}) ", *v, handle.id())
/// });
/// ```
///
/// The output will instead by `1(thread 1) 2(thread 1) 3(thread 1) 4(thread 1)
///  5(thread 2) 6(thread 2) 7(thread 2) 8(thread 2) 9(thread id2)`.
/// The reason for this is that Observable `b` emits its values directly like
/// before, but the emissions from `a` are scheduled on a new thread because we
/// are now using the `NewThread` Scheduler for that specific Observable.

pub trait SubscribeOn {
  fn subscribe_on<SD>(self, scheduler: SD) -> SubscribeOnOP<Self, SD>
  where
    Self: Sized,
  {
    SubscribeOnOP {
      source: self,
      scheduler,
    }
  }
}

pub struct SubscribeOnOP<S, SD> {
  source: S,
  scheduler: SD,
}

impl<T> SubscribeOn for T {}

impl<Item, Err, Sub, S, SD> RawSubscribable<Item, Err, Sub>
  for SubscribeOnOP<S, SD>
where
  Sub: Subscribe<Item, Err> + Send + 'static,
  S: RawSubscribable<Item, Err, Sub, Unsub = SharedSubscription>
    + Send
    + 'static,
  SD: Scheduler,
{
  type Unsub = SharedSubscription;

  fn raw_subscribe(self, subscribe: Sub) -> Self::Unsub {
    let source = self.source;
    self.scheduler.schedule(
      move |mut proxy, _: Option<()>| {
        proxy.add(Box::new(source.raw_subscribe(subscribe)))
      },
      None,
    )
  }
}

#[cfg(test)]
mod test {
  use crate::ops::{Delay, SubscribeOn};
  use crate::prelude::*;
  use crate::scheduler::Schedulers;
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;

  #[test]
  fn new_thread() {
    let res = Arc::new(Mutex::new(vec![]));
    let c_res = res.clone();
    let thread = Arc::new(Mutex::new(vec![]));
    let c_thread = thread.clone();
    observable::from_iter!(1..5)
      .subscribe_on(Schedulers::NewThread)
      .subscribe(move |v| {
        res.lock().unwrap().push(*v);
        let handle = thread::current();
        thread.lock().unwrap().push(handle.id());
      });

    thread::sleep(std::time::Duration::from_millis(1));
    assert_eq!(*c_res.lock().unwrap(), (1..5).collect::<Vec<_>>());
    assert_ne!(c_thread.lock().unwrap()[0], thread::current().id());
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
    observable::from_iter!(0..10)
      .subscribe_on(scheduler)
      .delay(Duration::from_millis(10))
      .subscribe(move |v| {
        emitted.lock().unwrap().push(*v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
