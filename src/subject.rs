use observer::observer_proxy_impl;

use crate::prelude::*;

mod mut_ref_subject;
pub use mut_ref_subject::*;

mod local_subject;
pub use local_subject::*;

mod shared_subject;
pub use shared_subject::*;

#[derive(Default, Clone)]
pub struct Subject<O, S> {
  pub(crate) observers: O,
  pub(crate) subscription: S,
}

impl<O, S> Subject<O, S>
where
  Self: Default,
{
  #[inline]
  pub fn new() -> Self { Self::default() }
}

subscription_proxy_impl!(Subject<O, U>, {subscription}, U, <O>);
observer_proxy_impl!(Subject<O, U>, {observers}, Item, Err, O, <U>, 
  {where Item: Clone, Err: Clone});

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::new();
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::new();
    broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!(e));
    broadcast.next(1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = Subject::new();
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn empty_local_subject_can_convert_to_shared() {
    use crate::scheduler::Schedulers;
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let mut subject = Subject::new();
    subject
      .clone()
      .to_shared()
      .observe_on(Schedulers::NewThread)
      .to_shared()
      .subscribe(move |v: i32| {
        *value.lock().unwrap() = v;
      });

    subject.next(100);
    std::thread::sleep(std::time::Duration::from_millis(1));

    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = LocalSubject::new();
    let local2 = LocalSubject::new();
    local.clone().actual_subscribe(Subscriber {
      observer: local2.observers,
      subscription: local2.subscription,
    });
    local.next(1);
    local.error(2);
  }
}
