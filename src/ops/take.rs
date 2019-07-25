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
/// # use rx_rs::{ ops::{Take}, prelude::*};
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

impl<'a, O> Take for O where O: ImplSubscribable {}

pub struct TakeOp<S> {
  source: S,
  count: u32,
}

impl<S> ImplSubscribable for TakeOp<S>
where
  S: ImplSubscribable,
{
  type Item = S::Item;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription> {
    let hit = Mutex::new(0);
    let count = self.count;
    self.source.subscribe_return_state(
      move |v| {
        let mut hit = hit.lock().unwrap();
        if *hit < count {
          *hit += 1;
          let os = next(v);
          match os {
            OState::Next => {
              if *hit == count {
                OState::Complete
              } else {
                os
              }
            }
            _ => os,
          }
        } else {
          OState::Complete
        }
      },
      error,
      complete,
    )
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
  use crate::prelude::*;
  use std::sync::{Arc, Mutex};

  #[test]
  fn base_function() {
    let completed = Arc::new(Mutex::new(false));
    let next_count = Arc::new(Mutex::new(0));
    let c_completed = completed.clone();
    let c_next_count = next_count.clone();

    observable::from_range(0..100).take(5).subscribe_complete(
      move |_| *next_count.lock().unwrap() += 1,
      move || *completed.lock().unwrap() = true,
    );

    assert_eq!(*c_next_count.lock().unwrap(), 5);
    assert_eq!(*c_completed.lock().unwrap(), true);
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
