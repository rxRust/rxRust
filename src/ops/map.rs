use crate::prelude::*;
use std::marker::PhantomData;

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<'a, T> {
  type Err;

  fn map<B, F>(self, f: F) -> MapOp<Self, NextWhitoutError<F, Self::Err>, B>
  where
    Self: Sized,
    F: Fn(T) -> B + 'a,
  {
    MapOp {
      source: self,
      func: NextWhitoutError::new(f),
      _p: PhantomData,
    }
  }

  fn map_with_err<B, F>(self, f: F) -> MapOp<Self, NextWithError<F>, B>
  where
    Self: Sized,
    F: Fn(T) -> Result<B, Self::Err> + 'a,
  {
    MapOp {
      source: self,
      func: NextWithError(f),
      _p: PhantomData,
    }
  }
}

impl<'a, T, O> Map<'a, T> for O
where
  O: Observable<'a>,
{
  type Err = O::Err;
}

pub struct MapOp<S, M, B> {
  source: S,
  func: M,
  _p: PhantomData<B>,
}

impl<'a, S, B, M> Observable<'a> for MapOp<S, M, B>
where
  M: WithErr<S::Item, B, Err = S::Err> + 'a,
  S: Observable<'a>,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item) -> OState<Self::Err>,
  {
    let func = self.func;
    self
      .source
      .subscribe_return_state(move |v| match func.call_with_err(v) {
        Ok(v) => next(v),
        Err(e) => OState::Err(e),
      })
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Map, prelude::*};
  use std::cell::Cell;

  #[test]
  fn primitive_type() {
    let i = Cell::new(0);
    let subject = Subject::<'_, i32, ()>::new();
    subject.clone().map(|v| v * 2).subscribe(|v| i.set(v));
    subject.next(100);
    assert_eq!(i.get(), 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let i = Cell::new(0);
    let subject = Subject::<'_, &i32, ()>::new();
    subject.clone().map(|v: &&i32| v).subscribe(|v| i.set(**v));
    subject.next(&100);
    assert_eq!(i.get(), 100);
  }

  #[test]
  fn unsubscribe() {
    let i = Cell::new(0);
    let subject = Subject::<'_, &i32, ()>::new();
    subject
      .clone()
      .map(|v: &&i32| v)
      .subscribe(|v| i.set(**v))
      .unsubscribe();
    subject.next(&100);
    assert_eq!(i.get(), 0);
  }

  #[test]
  #[should_panic]
  fn with_err() {
    let subject = Subject::new();

    subject
      .clone()
      .map_with_err(|_| Err("should panic "))
      .subscribe(|_: &i32| {})
      .on_error(|err| panic!(*err));

    subject.next(1);
  }
}
