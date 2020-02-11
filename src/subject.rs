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

subscription_proxy_impl!(Subject<O, U>, {subscription}, U, <O>);
observer_proxy_impl!(
  Subject<O, U>, {observers}, Item, Err, O, <U>, {where Item: Copy, Err: Copy});

#[cfg(test)]
mod test {
  use super::*;
  use crate::ops::{FilterMap, Map};

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::local();
      let _guard = broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::local();
    let _guard = broadcast
      .clone()
      .subscribe_err(|_: i32| {}, |e: _| panic!(e));
    broadcast.next(1);

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = Subject::local();
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn empty_local_subject_can_convert_to_shared() {
    use crate::{ops::ObserveOn, scheduler::Schedulers};
    use std::sync::{Arc, Mutex};
    let value = Arc::new(Mutex::new(0));
    let c_v = value.clone();
    let mut subject = Subject::shared();
    let _guard = subject.clone().observe_on(Schedulers::NewThread).subscribe(
      move |v: i32| {
        *value.lock().unwrap() = v;
      },
    );

    subject.next(100);
    std::thread::sleep(std::time::Duration::from_millis(1));

    assert_eq!(*c_v.lock().unwrap(), 100);
  }

  #[test]
  fn emit_mut_ref_life_time() {
    let mut i = 1;
    {
      // emit mut ref
      let mut subject = Subject::local();
      let _guard = subject
        .clone()
        .filter_map((|v| Some(v)) as for<'r> fn(&'r mut _) -> Option<&'r mut _>)
        .subscribe(|_: &mut i32| {
          i = 100;
        });
      subject.next(MutRefValue(&mut 1));
    }
    assert_eq!(i, 100);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = Subject::local();
    let local2 = Subject::local();
    local.clone().actual_subscribe(Subscriber {
      observer: local2.observers,
      subscription: local2.subscription,
    });
    local.next(1);
    local.error(2);
  }
}
