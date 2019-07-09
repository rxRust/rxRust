use crate::prelude::*;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rx_rs::{ops::Filter, prelude::*};
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// let coll = Rc::new(RefCell::new(vec![]));
/// let coll_clone = coll.clone();
///
/// observable::from_iter::<'_, _, _, ()>(0..10)
///   .filter(|v| *v % 2 == 0)
///   .subscribe(move |v| {
///      coll_clone.borrow_mut().push(*v);
///   });

/// // only even numbers received.
/// assert_eq!(coll.borrow().clone(), vec![0, 2, 4, 6, 8]);
/// ```

pub trait Filter<'a, T> {
  fn filter<N>(self, filter: N) -> FilterOp<Self, N>
  where
    Self: Sized,
    N: Fn(&T) -> bool + 'a,
  {
    FilterOp {
      source: self,
      filter,
    }
  }
}

pub trait FilterWithErr<'a, T> {
  type Err;
  fn filter_with_err<N>(self, filter: N) -> FilterWithErrOp<Self, N>
  where
    Self: Sized,
    N: Fn(&T) -> Result<bool, Self::Err> + 'a,
  {
    FilterWithErrOp {
      source: self,
      filter,
    }
  }
}

impl<'a, T, O> Filter<'a, T> for O where O: Subscribable<'a, Item = T> {}

impl<'a, T, O> FilterWithErr<'a, T> for O
where
  O: Subscribable<'a, Item = T>,
{
  type Err = O::Err;
}

pub struct FilterOp<S, N> {
  source: S,
  filter: N,
}

pub struct FilterWithErrOp<S, N> {
  source: S,
  filter: N,
}

impl<'a, S, F> Subscribable<'a> for FilterOp<S, F>
where
  S: Subscribable<'a>,
  F: Fn(&S::Item) -> bool + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsubscribable {
    let filter = self.filter;
    self.source.subscribe_return_state(
      move |v| {
        if filter(v) { next(v) } else { OState::Next }
      },
      error,
      complete,
    )
  }
}

impl<'a, S, F> Subscribable<'a> for FilterWithErrOp<S, F>
where
  S: Subscribable<'a>,
  F: Fn(&S::Item) -> Result<bool, S::Err> + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsubscribable {
    let filter = self.filter;
    self.source.subscribe_return_state(
      move |v| match filter(&v) {
        Ok(b) => {
          if b {
            next(v)
          } else {
            OState::Next
          }
        }
        Err(e) => OState::Err(e),
      },
      error,
      complete,
    )
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
    .subscribe_err(|_| {}, |err| panic!(*err));

  subject.next(&1);
}

#[test]
#[should_panic]
fn pass_error() {
  use crate::prelude::*;

  let mut subject = Subject::new();

  subject
    .clone()
    .filter(|_: &&i32| true)
    .subscribe_err(|_| {}, |err| panic!(*err));

  subject.error(&"");
}
