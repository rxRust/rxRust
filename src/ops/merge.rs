use crate::{Observable, Subscription};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// combine two Observables into one by merging their emissions
///
/// # Example
///
/// ```
/// # use rx_rs::{ ops::{Filter, Merge}, Observable, Observer, Subject };
/// let numbers = Subject::<'_, _, ()>::new();
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
  fn merge<S>(self, o: S) -> MergeOp<Self, S>
  where
    Self: Sized,
    S: Observable<'a, Item = T>,
  {
    MergeOp {
      source1: self,
      source2: o,
    }
  }
}

impl<'a, T, O> Merge<'a, T> for O where O: Observable<'a, Item = T> {}

pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}

impl<'a, T, E, S1: 'a, S2: 'a> Observable<'a> for MergeOp<S1, S2>
where
  S1: Observable<'a, Item = T, Err = E>,
  S2: Observable<'a, Item = T, Err = E>,
{
  type Err = E;
  type Item = T;
  type Unsubscribe = MergeSubscription<S1::Unsubscribe, S2::Unsubscribe>;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    let next = Rc::new(RefCell::new(next));
    let next_clone = next.clone();

    let s1 = self.source1.subscribe(move |v| {
      (&mut *next.borrow_mut())(v);
    });
    let s2 = self.source2.subscribe(move |v| {
      (&mut *next_clone.borrow_mut())(v);
    });

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

impl<'a, S1: 'a, S2: 'a, E> Subscription<'a> for MergeSubscription<S1, S2>
where
  S1: Subscription<'a, Err = E>,
  S2: Subscription<'a, Err = E>,
{
  type Err = E;
  fn on_error<OE>(&mut self, err: OE) -> &mut Self
  where
    OE: Fn(&Self::Err) + 'a,
  {
    let err = Rc::new(RefCell::new(err));

    let cb_err = err.clone();
    let stopped = self.stopped.clone();
    self.subscription1.on_error(move |e| {
      if !stopped.get() {
        cb_err.borrow()(e);
        stopped.set(true);
      }
    });

    let cb_err = err.clone();
    let stopped = self.stopped.clone();
    self.subscription2.on_error(move |e| {
      if !stopped.get() {
        cb_err.borrow()(e);
        stopped.set(true);
      }
    });
    self
  }
  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a,
  {
    let c = Rc::new(RefCell::new(complete));
    let completed = Rc::new(Cell::new(0));

    let completed_clone = completed.clone();
    let c2 = c.clone();
    self.subscription1.on_complete(move || {
      if completed_clone.get() == 1 {
        c2.borrow()()
      } else {
        completed_clone.set(1)
      }
    });

    self.subscription2.on_complete(move || {
      if completed.get() == 1 {
        c.borrow()()
      } else {
        completed.set(1)
      }
    });
    self
  }

  fn unsubscribe(self) {
    self.subscription1.unsubscribe();
    self.subscription2.unsubscribe();
  }

  fn throw_error(&self, err: &Self::Err) {
    self.subscription1.throw_error(&err);
    self.subscription2.throw_error(&err);
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Merge},
    Observable, Observer, Subject, Subscription,
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
    let even = numbers.clone().filter(|v| *v % 2 == 0).broadcast();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).broadcast();

    // merge odd and even stream again
    let merged = even.clone().merge(odd.clone());

    //  attach observers
    merged.subscribe(|v| numbers_store.borrow_mut().push(**v));
    odd.subscribe(|v| odd_store.borrow_mut().push(**v));
    even.subscribe(|v| even_store.borrow_mut().push(**v));

    (0..10).into_iter().for_each(|v| {
      numbers.next(v);
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

    numbers.next(1);
  }

  #[test]
  fn completed_test() {
    let even = Subject::<'_, _, ()>::new();
    let odd = Subject::<'_, _, ()>::new();

    let completed = Cell::new(false);
    even
      .clone()
      .merge(odd.clone())
      .subscribe(|_: &()| {})
      .on_complete(|| completed.set(true));

    even.complete();
    assert_eq!(completed.get(), false);
    odd.complete();
    assert_eq!(completed.get(), true);
  }

  #[test]

  fn error_test() {
    let even = Subject::new();
    let odd = Subject::new();

    let completed = Cell::new(0);
    let error = Cell::new(0);
    even
      .clone()
      .merge(odd.clone())
      .subscribe(|_: &()| {})
      .on_complete(|| completed.set(completed.get() + 1))
      .on_error(|_| error.set(error.get() + 1));

    odd.error(());
    even.clone().error(());
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(completed.get(), 0);
    // error should be hit just once
    assert_eq!(error.get(), 1);
  }
}
