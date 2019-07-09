use crate::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// combine two Observables into one by merging their emissions
///
/// # Example
///
/// ```
/// # use rx_rs::{ ops::{Filter, Merge}, prelude::*};
/// let numbers = Subject::<'_, i32, ()>::new();
/// // crate a even stream by filter
/// let even = numbers.clone().filter(|v| *v % 2 == 0);
/// // crate an odd stream by filter
/// let odd = numbers.clone().filter(|v| *v % 2 != 0);
///
/// // merge odd and even stream again
/// let merged = even.merge(odd);
///
/// // attach observers
/// merged.subscribe(|v| println!("{} ", v));
/// ```
pub trait Merge<'a, T> {
  type Err: 'a;
  fn merge<S>(self, o: S) -> MergeOp<Self, S>
  where
    Self: Sized,
    S: Subscribable<'a, Item = T, Err = Self::Err>,
  {
    MergeOp {
      source1: self,
      source2: o,
    }
  }
}

impl<'a, T, O> Merge<'a, T> for O
where
  O: Subscribable<'a, Item = T>,
{
  type Err = O::Err;
}

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

impl<'a, T, E: 'a, S1: 'a, S2: 'a> Subscribable<'a> for MergeOp<S1, S2>
where
  S1: Subscribable<'a, Item = T, Err = E>,
  S2: Subscribable<'a, Item = T, Err = E>,
{
  type Err = E;
  type Item = T;
  type Unsubscribable =
    MergeSubscription<S1::Unsubscribable, S2::Unsubscribable>;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsubscribable {
    let next = Rc::new(next);
    let next_clone = next.clone();

    let stopped = Rc::new(Cell::new(false));
    let error = error.map(Rc::new);
    let complete = complete.map(Rc::new);

    let on_error_factor = || {
      error.clone().map(|err| {
        let stopped = stopped.clone();
        move |e: &_| {
          if !stopped.get() {
            err(e);
            stopped.set(true);
          }
        }
      })
    };
    let completed = Rc::new(Cell::new(false));
    let on_complete_factor = || {
      complete.clone().map(|comp| {
        let stopped = stopped.clone();
        let completed = completed.clone();
        move || {
          if !stopped.get() && completed.get() {
            comp();
            stopped.set(true);
          } else {
            completed.set(true);
          }
        }
      })
    };

    let s1 = self.source1.subscribe_return_state(
      move |v| next(v),
      on_error_factor(),
      on_complete_factor(),
    );
    let s2 = self.source2.subscribe_return_state(
      move |v| next_clone(v),
      on_error_factor(),
      on_complete_factor(),
    );

    MergeSubscription::new(s1, s2)
  }
}

#[derive(Clone)]
pub struct MergeSubscription<S1, S2> {
  stopped: Rc<Cell<bool>>,
  subscription1: S1,
  subscription2: S2,
}

impl<S1, S2> MergeSubscription<S1, S2> {
  fn new(s1: S1, s2: S2) -> Self {
    MergeSubscription {
      stopped: Rc::new(Cell::new(false)),
      subscription1: s1,
      subscription2: s2,
    }
  }
}

impl<'a, S1: 'a, S2: 'a> Subscription for MergeSubscription<S1, S2>
where
  S1: Subscription,
  S2: Subscription,
{
  fn unsubscribe(&mut self) {
    self.subscription1.unsubscribe();
    self.subscription2.unsubscribe();
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Merge},
    prelude::*,
  };
  use std::cell::{Cell, RefCell};
  use std::rc::Rc;

  #[test]
  fn odd_even_merge() {
    // three collection to store streams emissions
    let odd_store = Rc::new(RefCell::new(vec![]));
    let even_store = Rc::new(RefCell::new(vec![]));
    let numbers_store = Rc::new(RefCell::new(vec![]));

    let numbers = Subject::<'_, _, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| v % 2 == 0).broadcast();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).broadcast();

    // merge odd and even stream again
    let merged = even.clone().merge(odd.clone());

    //  attach observers
    merged.subscribe(|v| numbers_store.borrow_mut().push(*v));
    odd.subscribe(|v| odd_store.borrow_mut().push(*v));
    even.subscribe(|v| even_store.borrow_mut().push(*v));

    (0..10).for_each(|v| {
      numbers.next(&v);
    });

    assert_eq!(even_store.borrow().clone(), vec![0, 2, 4, 6, 8]);
    assert_eq!(odd_store.borrow().clone(), vec![1, 3, 5, 7, 9]);
    assert_eq!(numbers_store.borrow().clone(), (0..10).collect::<Vec<_>>());
  }

  #[test]
  fn merge_unsubscribe_work() {
    let numbers = Subject::<'_, _, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0).broadcast();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).broadcast();

    even
      .merge(odd)
      .subscribe(|_| unreachable!("oh, unsubscribe not work."))
      .unsubscribe();

    numbers.next(&1);
  }

  #[test]
  fn completed_test() {
    let completed = Cell::new(false);
    let mut even = Subject::<'_, _, ()>::new();
    let mut odd = Subject::<'_, _, ()>::new();

    even
      .clone()
      .merge(odd.clone())
      .subscribe_complete(|_: &()| {}, || completed.set(true));

    even.complete();
    assert_eq!(completed.get(), false);
    odd.complete();
    assert_eq!(completed.get(), true);
    completed.set(false);
    even.complete();
    assert_eq!(completed.get(), false);
  }

  #[test]
  fn error_test() {
    let completed = Cell::new(0);
    let error = Cell::new(0);
    let mut even = Subject::new();
    let mut odd = Subject::new();

    even.clone().merge(odd.clone()).subscribe_err_complete(
      |_: &()| {},
      |_| error.set(error.get() + 1),
      || completed.set(completed.get() + 1),
    );

    odd.error(&"");
    even.clone().error(&"");
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(completed.get(), 0);
    // error should be hit just once
    assert_eq!(error.get(), 1);
  }
}
