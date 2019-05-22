use crate::{ErrComplete, Observable, Subscription};
use std::cell::RefCell;
use std::rc::Rc;

/// combine two Observables into one by merging their emissions
///
/// # Example
///
/// ```
/// # use rx_rs::{
///   ops::{Filter, Merge}, Observable, Observer, Subject, ErrComplete
///  };
/// let numbers = Subject::new();
/// // crate a even stream by filter
/// let even = numbers.clone().filter(|v| *v % 2 == 0);
/// // crate an odd stream by filter
/// let odd = numbers.clone().filter(|v| *v % 2 != 0);
///
/// // merge odd and even stream again
/// let merged = even.merge(odd);
///
/// // attach observers
/// merged.subscribe(|v| println!("{} ", v), |ec: &ErrComplete<()>| {});
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

impl<'a, T, S1, S2, E> Observable<'a> for MergeOp<S1, S2>
where
  S1: Observable<'a, Item = T, Err = E>,
  S2: Observable<'a, Item = T, Err = E>,
{
  type Item = T;
  type Unsubscribe = MergeSubscription<S1::Unsubscribe, S2::Unsubscribe>;
  type Err = E;

  fn subscribe<N, EC>(self, next: N, err_or_complete: EC) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
    EC: 'a + Fn(&ErrComplete<Self::Err>),
  {
    let next = Rc::new(RefCell::new(next));
    let next_clone = next.clone();
    ;
    let on_ec1 = Rc::new(RefCell::new(err_or_complete));
    let on_ec2 = on_ec1.clone();

    let subscription1 = self.source1.subscribe(
      move |v| {
        (&mut *next.borrow_mut())(v);
      },
      move |ec| (&mut *on_ec1.borrow_mut())(ec),
    );
    let subscription2 = self.source2.subscribe(
      move |v| {
        (&mut *next_clone.borrow_mut())(v);
      },
      move |ec| (&mut *on_ec2.borrow_mut())(ec),
    );

    MergeSubscription {
      subscription1,
      subscription2,
    }
  }
}

pub struct MergeSubscription<S1, S2> {
  subscription1: S1,
  subscription2: S2,
}

impl<S1, S2> Subscription for MergeSubscription<S1, S2>
where
  S1: Subscription,
  S2: Subscription,
{
  fn unsubscribe(self) {
    self.subscription1.unsubscribe();
    self.subscription2.unsubscribe();
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, Merge},
    ErrComplete, Observable, Observer, Subject, Subscription,
  };
  use std::cell::RefCell;
  use std::rc::Rc;

  #[test]
  fn odd_even_merge() {
    // three collection to store streams emissions
    let odd_store = Rc::new(RefCell::new(vec![]));
    let even_store = Rc::new(RefCell::new(vec![]));
    let numbers_store = Rc::new(RefCell::new(vec![]));

    let numbers = Subject::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0).broadcast();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).broadcast();

    // merge odd and even stream again
    let merged = even.clone().merge(odd.clone());

    //  attach observers
    merged.subscribe(
      |v| numbers_store.borrow_mut().push(**v),
      |_ec: &ErrComplete<()>| {},
    );
    odd.subscribe(
      |v| odd_store.borrow_mut().push(**v),
      |_ec: &ErrComplete<()>| {},
    );
    even.subscribe(
      |v| even_store.borrow_mut().push(**v),
      |_ec: &ErrComplete<()>| {},
    );

    (0..10).into_iter().for_each(|v| {
      numbers.next(v);
    });

    assert_eq!(even_store.borrow().clone(), vec![0, 2, 4, 6, 8]);
    assert_eq!(odd_store.borrow().clone(), vec![1, 3, 5, 7, 9]);
    assert_eq!(numbers_store.borrow().clone(), (0..10).collect::<Vec<_>>());
  }

  #[test]
  fn merge_unsubscribe_work() {
    let numbers = Subject::new();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0).broadcast();
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0).broadcast();

    even
      .merge(odd)
      .subscribe(
        |_| unreachable!("oh, unsubscribe not work."),
        |_ec: &ErrComplete<()>| {},
      )
      .unsubscribe();

    numbers.next(1);
  }

}
