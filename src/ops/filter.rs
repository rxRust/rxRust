use crate::{NextObserver, NextWhitoutError, NextWithError, Observable};

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rx_rs::{ops::Filter, Subject, Observable, Observer};
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// let subject = Subject::<'_, _, ()>::new();
/// let coll = Rc::new(RefCell::new(vec![]));
/// let coll_clone = coll.clone();
///
/// subject.clone().filter(|v| *v % 2 == 0).subscribe(move |v| {
///    coll_clone.borrow_mut().push(*v);
/// });

/// (0..10).into_iter().for_each(|v| {
///    subject.next(v);
/// });

/// // only even numbers received.
/// assert_eq!(coll.borrow().clone(), vec![0, 2, 4, 6, 8]);
/// ```
use std::{cell::RefCell, rc::Rc};


pub trait Filter<'a, T> {
  type Err;
  fn filter<N>(self, filter: N) -> FilterOp<Self, NextWhitoutError<N>>
  where
    Self: Sized,
    N: Fn(&T) -> bool + 'a,
  {
    FilterOp {
      source: self,
      filter: NextWhitoutError(filter),
    }
  }

  fn filter_with_err<N>(self, filter: N) -> FilterOp<Self, NextWithError<N>>
  where
    Self: Sized,
    N: Fn(&T) -> Result<bool, Self::Err> + 'a,
  {
    FilterOp {
      source: self,
      filter: NextWithError(filter),
    }
  }
}

impl<'a, T, O> Filter<'a, T> for O
where
  O: Observable<'a, Item = T>,
{
  type Err = O::Err;
}

pub struct FilterOp<S, N> {
  source: S,
  filter: N,
}

impl<'a, S, F> Observable<'a> for FilterOp<S, F>
where
  S: Observable<'a>,
  F: NextObserver<S::Item, bool, Err = S::Err> + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    let filter = self.filter;
    let subscription: Rc<RefCell<Option<S::Unsubscribe>>> =
      Rc::new(RefCell::new(None));
    let sc = subscription.clone();

    let s = self.source.subscribe(move |v| {
      let subscription = subscription.borrow_mut();
      let subscription = subscription.as_ref().unwrap();
      if let Some(b) = filter.call_and_consume_err(&v, subscription) {
        if b {
          next(v)
        };
      }
    });
    sc.replace(Some(s.clone()));
    s
  }
}

