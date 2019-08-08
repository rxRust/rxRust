use crate::prelude::*;
use std::sync::Arc;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rxrust::{ops::Filter, prelude::*};
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

impl<'a, T, O> Filter<T> for O where O: RawSubscribable<Item = T> {}

impl<'a, T, O> FilterWithErr<'a, T> for O
where
  O: RawSubscribable<Item = T>,
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

impl<S, F> RawSubscribable for FilterOp<S, F>
where
  S: RawSubscribable,
  F: RxFn(&S::Item) -> bool + Send + Sync + 'static,
{
  type Err = S::Err;
  type Item = S::Item;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let filter = self.filter;
    self.source.raw_subscribe(RxFnWrapper::new(
      move |v: RxValue<&'_ _, &'_ _>| match v {
        RxValue::Next(ne) => {
          if filter.call((ne,)) {
            subscribe.call((RxValue::Next(ne),))
          } else {
            RxReturn::Continue
          }
        }
        vv => subscribe.call((vv,)),
      },
    ))
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

impl<S, F> RawSubscribable for FilterWithErrOp<S, F>
where
  S: RawSubscribable,
  F: RxFn(&S::Item) -> Result<bool, S::Err> + Send + Sync + 'static,
{
  type Err = S::Err;
  type Item = S::Item;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let filter = self.filter;
    self.source.raw_subscribe(RxFnWrapper::new(
      move |v: RxValue<&'_ _, &'_ _>| match v {
        RxValue::Next(nv) => match filter.call((&nv,)) {
          Ok(b) => {
            if b {
              subscribe.call((RxValue::Next(nv),))
            } else {
              RxReturn::Continue
            }
          }
          Err(e) => RxReturn::Err(e),
        },
        vv => subscribe.call((vv,)),
      },
    ))
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

#[cfg(test)]
mod test {
  use crate::{
    ops::{Filter, FilterWithErr},
    prelude::*,
  };

  #[test]
  #[should_panic]
  fn runtime_error() {
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
    let mut subject = Subject::new();

    subject
      .clone()
      .filter(|_: &&i32| true)
      .subscribe_err(|_| {}, |err| panic!(*err));

    subject.error(&"");
  }

  #[test]
  fn test_fork() {
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
}
