use crate::prelude::*;
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
/// let numbers = Subject::<'_, _, ()>::new();
/// numbers.clone().take(5).subscribe(|v| println!("{}", v));
///
/// (0..10).into_iter().for_each(|v| {
///    numbers.next(&v);
/// });

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

pub struct TakeOp<S> {
  source: S,
  count: u32,
}

impl<'a, S> ImplSubscribable<'a> for TakeOp<S>
where
  S: ImplSubscribable<'a> + 'a,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    let total = self.count;
    let count = std::cell::Cell::new(0);
    self.source.subscribe_return_state(
      move |v| {
        if count.get() < total {
          count.set(count.get() + 1);
          let os = next(v);
          match os {
            OState::Next => {
              if count.get() == total {
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

impl<'a, O> Take for O where O: ImplSubscribable<'a> {}

#[cfg(test)]
mod test {
  use super::Take;
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn base_function() {
    let completed = Cell::new(false);
    let next_count = Cell::new(0);

    observable::from_iter::<'_, _, _, ()>(0..10)
      .take(5)
      .subscribe_complete(
        |_| next_count.set(next_count.get() + 1),
        || completed.set(true),
      );

    assert_eq!(completed.get(), true);
    assert_eq!(next_count.get(), 5);
  }
}
