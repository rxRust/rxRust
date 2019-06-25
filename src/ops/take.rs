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

impl<'a, S> Observable<'a> for TakeOp<S>
where
  S: Observable<'a> + 'a,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe_return_state<N>(self, mut next: N) -> Self::Unsubscribe
  where
    N: 'a + FnMut(&Self::Item) -> OState<Self::Err>,
  {
    let total = self.count;
    let mut count = 0;
    self.source.subscribe_return_state(move |v| {
      if count < total {
        count += 1;
        let os = next(v);
        match os {
          OState::Next => {
            if count == total {
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
    })
  }
}

impl<'a, O> Take for O where O: Observable<'a> {}

#[cfg(test)]
mod test {
  use super::Take;
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn base_function() {
    let completed = Cell::new(false);
    let next_count = Cell::new(0);

    let numbers = Subject::<'_, _, ()>::new();
    numbers
      .clone()
      .take(5)
      .subscribe(|_| next_count.set(next_count.get() + 1))
      .on_complete(|| completed.set(true));

    (0..10).for_each(|v| {
      numbers.next(&v);
    });

    assert_eq!(completed.get(), true);
    assert_eq!(next_count.get(), 5);
  }
}
