use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::sync::mpsc::channel;

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

impl<S> SubscribeOn for S where S: RawSubscribable {}

impl<S, SD> RawSubscribable for SubscribeOnOP<S, SD>
where
  S: RawSubscribable + Send + Sync + 'static,
  SD: Scheduler,
{
  type Item = S::Item;
  type Err = S::Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let (sender, receiver) = channel();
    let source = self.source;
    self.scheduler.schedule(
      move |_: Option<()>| {
        sender
          .send(source.raw_subscribe(subscribe))
          .expect("send result from SubscribeOn failed.")
      },
      None,
    );
    receiver.recv().unwrap()
  }
}

#[test]
fn new_thread() {
  use crate::ops::{Merge, SubscribeOn};
  use crate::prelude::*;
  use crate::scheduler::Schedulers;
  use std::sync::{Arc, Mutex};
  use std::thread;

  let res = Arc::new(Mutex::new(vec![]));
  let c_res = res.clone();
  let a = observable::from_range(1..5).subscribe_on(Schedulers::NewThread);
  let b = observable::from_range(5..10);
  let thread = Arc::new(Mutex::new(vec![]));
  let c_thread = thread.clone();
  a.merge(b).subscribe(move |v| {
    res.lock().unwrap().push(*v);
    let handle = thread::current();
    thread.lock().unwrap().push(handle.id());
  });

  assert_eq!(*c_res.lock().unwrap(), (1..10).collect::<Vec<_>>());
  let first = c_thread.lock().unwrap()[0];
  let second = c_thread.lock().unwrap()[4];
  assert_ne!(first, second);
  let mut thread_list = vec![first; 4];
  thread_list.append(&mut vec![second; 5]);
  assert_eq!(*c_thread.lock().unwrap(), thread_list);
}
