use crate::prelude::*;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rx_rs::{ops::Filter, prelude::*};
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

pub trait Filter<'a, T> {
  type Err;
  fn filter<N>(
    self, filter: N,
  ) -> FilterOp<Self, NextWhitoutError<N, Self::Err>>
  where
    Self: Sized,
    N: Fn(&T) -> bool + 'a,
  {
    FilterOp {
      source: self,
      filter: NextWhitoutError::new(filter),
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
  F: WithErrByRef<S::Item, bool, Err = S::Err> + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe_with_err<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item) -> Option<Self::Err>,
  {
    let filter = self.filter;
    self
      .source
      .subscribe_with_err(move |v| match filter.call_with_err(&v) {
        Ok(b) => {
          if b {
            next(v)
          } else {
            None
          }
        }
        Err(e) => Some(e),
      })
  }
}

#[test]
#[should_panic]
fn runtime_error() {
  use crate::prelude::*;

  let subject = Subject::new();

  subject
    .clone()
    .filter_with_err(|_| Err("runtime error"))
    .subscribe(|_| {})
    .on_error(|err| panic!(*err));

  subject.next(1);
}

#[test]
#[should_panic]
fn pass_error() {
  use crate::prelude::*;

  let subject = Subject::new();

  subject
    .clone()
    .filter(|_: &&i32| true)
    .subscribe(|_| {})
    .on_error(|err| panic!(*err));

  subject.error("");
}
