use crate::prelude::*;
use std::sync::Mutex;
/// Emits only the first `count` values emitted by the source Observable.
///
/// `take` returns an Observable that emits only the first `count` values
/// emitted by the source Observable. If the source emits fewer than `count`
/// values then all of its values are emitted. After that, it completes,
/// regardless if the source completes.
///
/// # Example
/// Take the first 5 seconds of an infinite 1-second interval Observable
///
/// ```
/// # use rxrust::{
///   ops::{Take}, prelude::*,
///   subscribable::Subscribable
/// };
///
/// observable::from_range(0..10).take(5).subscribe(|v| println!("{}", v));
///

/// // print logs:
/// // 0
/// // 1
/// // 2
/// // 3
/// // 4
/// ```
///
pub trait Take {
  fn take(self, count: u32) -> TakeOp<Self>
  where
    Self: Sized,
  {
    TakeOp {
      source: self,
      count,
    }
  }
}

impl<'a, O> Take for O where O: RawSubscribable {}

pub struct TakeOp<S> {
  source: S,
  count: u32,
}

impl<S> RawSubscribable for TakeOp<S>
where
  S: RawSubscribable,
{
  type Item = S::Item;
  type Err = S::Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let hit = Mutex::new(0);
    let count = self.count;
    self.source.raw_subscribe(RxFnWrapper::new(
      move |v: RxValue<&'_ _, &'_ _>| match v {
        RxValue::Next(nv) => {
          let mut hit = hit.lock().unwrap();
          if *hit < count {
            *hit += 1;
            let os = subscribe.call((RxValue::Next(nv),));
            if let RxReturn::Continue = os {
              if *hit == count {
                return RxReturn::Complete;
              }
            }
            os
          } else {
            RxReturn::Continue
          }
        }
        vv => subscribe.call((vv,)),
      },
    ))
  }
}

impl<S> Multicast for TakeOp<S>
where
  S: Multicast,
{
  type Output = TakeOp<S::Output>;
  fn multicast(self) -> Self::Output {
    TakeOp {
      source: self.source.multicast(),
      count: self.count,
    }
  }
}

impl<S> Fork for TakeOp<S>
where
  S: Fork,
{
  type Output = TakeOp<S::Output>;
  fn fork(&self) -> Self::Output {
    TakeOp {
      source: self.source.fork(),
      count: self.count,
    }
  }
}

#[cfg(test)]
mod test {
  use super::Take;
  use crate::{prelude::*, subscribable::Subscribable};
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn base_function() {
    let completed = Arc::new(AtomicBool::new(false));
    let next_count = Arc::new(Mutex::new(0));
    let c_completed = completed.clone();
    let c_next_count = next_count.clone();

    observable::from_range(0..100).take(5).subscribe_complete(
      move |_| *next_count.lock().unwrap() += 1,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(*c_next_count.lock().unwrap(), 5);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn take_support_fork() {
    let nc1 = Arc::new(Mutex::new(0));
    let nc2 = Arc::new(Mutex::new(0));
    let c_nc1 = nc1.clone();
    let c_nc2 = nc2.clone();
    let take5 = observable::from_range(0..100).take(5).multicast();
    let f1 = take5.fork();
    let f2 = take5.fork();
    f1.take(5)
      .fork()
      .subscribe(move |_| *nc1.lock().unwrap() += 1);
    f2.take(5)
      .fork()
      .subscribe(move |_| *nc2.lock().unwrap() += 1);

    assert_eq!(*c_nc1.lock().unwrap(), 5);
    assert_eq!(*c_nc2.lock().unwrap(), 5);
  }
}
