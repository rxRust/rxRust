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
pub trait Merge<'a> {
  fn merge<S>(self, o: S) -> MergeOp<Self, S>
  where
    Self: Sized,
  {
    MergeOp {
      source1: self,
      source2: o,
    }
  }
}

impl<'a, 'b, O> Merge<'b> for O where O: ImplSubscribable<'a> {}

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

fn merge_stream<'a, S1, S2>(
  s1: S1,
  s2: S2,
  next: impl Fn(&S1::Item) -> OState<S1::Err> + 'a,
  error: Option<impl Fn(&S1::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Box<dyn Subscription + 'a>
where
  S1: ImplSubscribable<'a>,
  S2: ImplSubscribable<'a, Item = S1::Item, Err = S1::Err>,
{
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

  let unsub1 = s1.subscribe_return_state(
    move |v| next(v),
    on_error_factor(),
    on_complete_factor(),
  );
  let unsub2 = s2.subscribe_return_state(
    move |v| next_clone(v),
    on_error_factor(),
    on_complete_factor(),
  );

  Box::new(MergeSubscription::new(unsub1, unsub2))
}

impl<'a, S1, S2> ImplSubscribable<'a> for MergeOp<S1, S2>
where
  S1: ImplSubscribable<'a>,
  S2: ImplSubscribable<'a, Item = S1::Item, Err = S1::Err>,
{
  type Err = S1::Err;
  type Item = S1::Item;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    merge_stream(self.source1, self.source2, next, error, complete)
  }
}

impl<'a, S1, S2> ImplSubscribable<'a> for &'a MergeOp<S1, S2>
where
  &'a S1: ImplSubscribable<'a>,
  &'a S2: ImplSubscribable<
    'a,
    Item = <&'a S1 as ImplSubscribable<'a>>::Item,
    Err = <&'a S1 as ImplSubscribable<'a>>::Err,
  >,
{
  type Err = <&'a S1 as ImplSubscribable<'a>>::Err;
  type Item = <&'a S1 as ImplSubscribable<'a>>::Item;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    merge_stream(&self.source1, &self.source2, next, error, complete)
  }
}

pub struct MergeSubscription<'a> {
  subscription1: Box<dyn Subscription + 'a>,
  subscription2: Box<dyn Subscription + 'a>,
}

impl<'a> MergeSubscription<'a> {
  fn new(
    s1: Box<dyn Subscription + 'a>,
    s2: Box<dyn Subscription + 'a>,
  ) -> Self {
    MergeSubscription {
      subscription1: s1,
      subscription2: s2,
    }
  }
}

impl<'a> Subscription for MergeSubscription<'a> {
  fn unsubscribe(&mut self) {
    self.subscription1.unsubscribe();
    self.subscription2.unsubscribe();
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Fork, Merge, ToSubject},
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

    let numbers = Subject::<_, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| v % 2 == 0).to_subject();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).to_subject();

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
    let numbers = Subject::<_, ()>::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0);
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0);

    even
      .merge(odd)
      .subscribe(|_| unreachable!("oh, unsubscribe not work."))
      .unsubscribe();

    numbers.next(&1);
  }

  #[test]
  fn completed_test() {
    let completed = Rc::new(Cell::new(false));
    let c_clone = completed.clone();
    let mut even = Subject::<_, ()>::new();
    let mut odd = Subject::<_, ()>::new();

    even
      .clone()
      .merge(odd.clone())
      .subscribe_complete(|_: &()| {}, move || completed.set(true));

    even.complete();
    assert_eq!(c_clone.get(), false);
    odd.complete();
    assert_eq!(c_clone.get(), true);
    c_clone.set(false);
    even.complete();
    assert_eq!(c_clone.get(), false);
  }

  #[test]
  fn error_test() {
    let completed = Rc::new(Cell::new(0));
    let cc = completed.clone();
    let error = Rc::new(Cell::new(0));
    let ec = error.clone();
    let mut even = Subject::new();
    let mut odd = Subject::new();

    even.clone().merge(odd.clone()).subscribe_err_complete(
      |_: &()| {},
      move |_| error.set(error.get() + 1),
      move || completed.set(completed.get() + 1),
    );

    odd.error(&"");
    even.clone().error(&"");
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(cc.get(), 0);
    // error should be hit just once
    assert_eq!(ec.get(), 1);
  }

  #[test]
  fn merge_fork() {
    let o = Observable::new(|s| {
      s.next(&1);
      s.next(&2);
      s.error(&());
    });

    let m = o.fork().merge(o.fork());
    m.fork().merge(m.fork());
  }
}
