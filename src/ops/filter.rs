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

impl<'a, T, O> Filter<T> for O where O: RawSubscribable<Item = T> {}

pub struct FilterOp<S, N> {
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
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
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

#[cfg(test)]
mod test {
  use crate::{ops::Filter, prelude::*};

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
  }
}
