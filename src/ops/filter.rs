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

impl<'a, T, O> Filter<'a, T> for O where O: ImplSubscribable<'a, Item = T> {}

impl<'a, T, O> FilterWithErr<'a, T> for O
where
  O: ImplSubscribable<'a, Item = T>,
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

#[inline]
fn subscribe_source<'a, S, F>(
  source: S,
  filter: F,
  next: impl Fn(&S::Item) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> S::Unsub
where
  S: ImplSubscribable<'a>,
  F: Fn(&S::Item) -> bool + 'a,
{
  source.subscribe_return_state(
    move |v| {
      if filter(v) { next(v) } else { OState::Next }
    },
    error,
    complete,
  )
}

impl<'a, S, F> ImplSubscribable<'a> for FilterOp<S, F>
where
  S: ImplSubscribable<'a>,
  F: Fn(&S::Item) -> bool + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsub = S::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source(self.source, self.filter, next, error, complete)
  }
}

impl<'a, S, F> ImplSubscribable<'a> for &'a FilterOp<S, F>
where
  &'a S: ImplSubscribable<'a>,
  F: Fn(&<&'a S as ImplSubscribable<'a>>::Item) -> bool + 'a,
{
  type Err = <&'a S as ImplSubscribable<'a>>::Err;
  type Item = <&'a S as ImplSubscribable<'a>>::Item;
  type Unsub = <&'a S as ImplSubscribable<'a>>::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source(&self.source, &self.filter, next, error, complete)
  }
}

#[inline]
fn subscribe_source_with_err<'a, S, F>(
  source: S,
  filter: F,
  next: impl Fn(&S::Item) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> S::Unsub
where
  S: ImplSubscribable<'a>,
  F: Fn(&S::Item) -> Result<bool, S::Err> + 'a,
{
  source.subscribe_return_state(
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

impl<'a, S, F> ImplSubscribable<'a> for FilterWithErrOp<S, F>
where
  S: ImplSubscribable<'a>,
  F: Fn(&S::Item) -> Result<bool, S::Err> + 'a,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsub = S::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source_with_err(self.source, self.filter, next, error, complete)
  }
}

impl<'a, S, F> ImplSubscribable<'a> for &'a FilterWithErrOp<S, F>
where
  &'a S: ImplSubscribable<'a>,
  F: Fn(
      &<&'a S as ImplSubscribable<'a>>::Item,
    ) -> Result<bool, <&'a S as ImplSubscribable<'a>>::Err>
    + 'a,
{
  type Err = <&'a S as ImplSubscribable<'a>>::Err;
  type Item = <&'a S as ImplSubscribable<'a>>::Item;
  type Unsub = <&'a S as ImplSubscribable<'a>>::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source_with_err(&self.source, &self.filter, next, error, complete)
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

#[test]
fn test_fork() {
  use crate::ops::Fork;
  use crate::prelude::*;
  let obser = observable::from_iter(0..10).filter(|v| v % 2 == 0);
  let _f1 = obser.fork();
  let _f2 = obser.fork();

  // filter with error
  let obser = observable::from_iter(0..10).filter_with_err(|v| Ok(v % 2 == 0));
  let _f1 = obser.fork();
  let _f2 = obser.fork();
}
