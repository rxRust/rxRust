use crate::prelude::*;
use std::sync::Arc;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rx_rs::{ops::Filter, prelude::*};
/// use std::sync::{Arc, Mutex};
///
/// let coll = Arc::new(Mutex::new(vec![]));
/// let coll_clone = coll.clone();
///
/// observable::from_range(0..10)
///   .filter(|v| *v % 2 == 0)
///   .subscribe(move |v| {
///      coll_clone.lock().unwrap().push(*v);
///   });

/// // only even numbers received.
/// assert_eq!(*coll.lock().unwrap(), vec![0, 2, 4, 6, 8]);
/// ```

pub trait Filter<T> {
  fn filter<N>(self, filter: N) -> FilterOp<Self, RxFnWrapper<N>>
  where
    Self: Sized,
    N: Fn(&T) -> bool,
  {
    FilterOp {
      source: self,
      filter: RxFnWrapper::new(filter),
    }
  }
}

pub trait FilterWithErr<'a, T> {
  type Err;
  fn filter_with_err<N>(
    self,
    filter: N,
  ) -> FilterWithErrOp<Self, RxFnWrapper<N>>
  where
    Self: Sized,
    N: Fn(&T) -> Result<bool, Self::Err> + 'a,
  {
    FilterWithErrOp {
      source: self,
      filter: RxFnWrapper::new(filter),
    }
  }
}

impl<'a, T, O> Filter<T> for O where O: ImplSubscribable<Item = T> {}

impl<'a, T, O> FilterWithErr<'a, T> for O
where
  O: ImplSubscribable<Item = T>,
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

impl<S, F> ImplSubscribable for FilterOp<S, F>
where
  S: ImplSubscribable,
  F: RxFn(&S::Item) -> bool + Send + Sync + 'static,
{
  type Err = S::Err;
  type Item = S::Item;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription> {
    let filter = self.filter;
    self.source.subscribe_return_state(
      move |v| {
        if filter.call((v,)) {
          next(v)
        } else {
          OState::Next
        }
      },
      error,
      complete,
    )
  }
}

impl<S, F> Multicast for FilterOp<S, F>
where
  S: Multicast,
  F: RxFn(&S::Item) -> bool + Send + Sync + 'static,
{
  type Output = FilterOp<S::Output, Arc<F>>;
  fn multicast(self) -> Self::Output {
    FilterOp {
      source: self.source.multicast(),
      filter: Arc::new(self.filter),
    }
  }
}

impl<S, F> Fork for FilterOp<S, Arc<F>>
where
  S: Fork,
  F: RxFn(&S::Item) -> bool + Send + Sync + 'static,
{
  type Output = FilterOp<S::Output, Arc<F>>;
  fn fork(&self) -> Self::Output {
    FilterOp {
      source: self.source.fork(),
      filter: self.filter.clone(),
    }
  }
}

impl<S, F> ImplSubscribable for FilterWithErrOp<S, F>
where
  S: ImplSubscribable,
  F: RxFn(&S::Item) -> Result<bool, S::Err> + Send + Sync + 'static,
{
  type Err = S::Err;
  type Item = S::Item;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription> {
    let filter = self.filter;
    self.source.subscribe_return_state(
      move |v| match filter.call((&v,)) {
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

impl<S, F> Multicast for FilterWithErrOp<S, F>
where
  S: Multicast,
  F: RxFn(&S::Item) -> Result<bool, S::Err> + Send + Sync + 'static,
{
  type Output = FilterWithErrOp<S::Output, Arc<F>>;
  fn multicast(self) -> Self::Output {
    FilterWithErrOp {
      source: self.source.multicast(),
      filter: Arc::new(self.filter),
    }
  }
}

impl<S, F> Fork for FilterWithErrOp<S, Arc<F>>
where
  S: Fork,
  F: RxFn(&S::Item) -> Result<bool, S::Err> + Send + Sync + 'static,
{
  type Output = FilterWithErrOp<S::Output, Arc<F>>;
  fn fork(&self) -> Self::Output {
    FilterWithErrOp {
      source: self.source.fork(),
      filter: self.filter.clone(),
    }
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
  use crate::prelude::*;
  observable::from_range(0..10)
    .filter(|v| v % 2 == 0)
    .multicast()
    .fork()
    .filter(|_| true)
    .multicast()
    .fork()
    .subscribe(|_| {});

  // filter with error
  observable::from_range(0..10)
    .filter_with_err(|_| Ok(true))
    .multicast()
    .fork()
    .filter_with_err(|_| Ok(true))
    .multicast()
    .fork()
    .subscribe(|_| {});
}
