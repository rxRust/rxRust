use crate::prelude::*;

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<'a, T> {
  fn map<B, F>(self, f: F) -> MapOp<Self, F>
  where
    Self: Sized,
    F: Fn(&T) -> B + 'a,
  {
    MapOp {
      source: self,
      func: f,
    }
  }

  /// A version of map extension which return reference, and furthermore，return
  /// type and input item has same lifetime.
  fn map_return_ref<B, F>(self, f: F) -> MapReturnRefOp<Self, F>
  where
    Self: Sized,
    F: for<'r> Fn(&'r T) -> &'r B + 'a,
  {
    MapReturnRefOp {
      source: self,
      func: f,
    }
  }
}

pub trait MapWithErr<'a, T> {
  type Err;
  fn map_with_err<B, F>(self, f: F) -> MapWithErrOp<Self, F>
  where
    Self: Sized,
    F: Fn(&T) -> Result<B, Self::Err> + 'a,
  {
    MapWithErrOp {
      source: self,
      func: f,
    }
  }

  /// A version of map_with_err extension which return reference, and
  /// furthermore， return item and input item has same lifetime.
  fn map_return_ref_with_err<B, F>(self, f: F) -> MapReturnRefWithErrOp<Self, F>
  where
    Self: Sized,
    F: for<'r> Fn(&'r T) -> Result<&'r B, Self::Err> + 'a,
  {
    MapReturnRefWithErrOp {
      source: self,
      func: f,
    }
  }
}

impl<'a, O> Map<'a, O::Item> for O where O: Subscribable<'a> {}

impl<'a, O> MapWithErr<'a, O::Item> for O
where
  O: Subscribable<'a>,
{
  type Err = O::Err;
}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> Subscribable<'a> for MapOp<S, M>
where
  M: Fn(&S::Item) -> B + 'a,
  S: Subscribable<'a>,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    let func = self.func;
    self.source.subscribe_return_state(move |v| next(&func(v)))
  }
}

pub struct MapWithErrOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> Subscribable<'a> for MapWithErrOp<S, M>
where
  M: Fn(&S::Item) -> Result<B, S::Err> + 'a,
  S: Subscribable<'a>,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    let func = self.func;
    self.source.subscribe_return_state(move |v| match func(v) {
      Ok(v) => next(&v),
      Err(e) => OState::Err(e),
    })
  }
}

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> Subscribable<'a> for MapReturnRefOp<S, M>
where
  M: for<'r> Fn(&'r S::Item) -> &'r B + 'a,
  S: Subscribable<'a>,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    let func = self.func;
    self.source.subscribe_return_state(move |v| next(&func(v)))
  }
}

pub struct MapReturnRefWithErrOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> Subscribable<'a> for MapReturnRefWithErrOp<S, M>
where
  M: for<'r> Fn(&'r S::Item) -> Result<&'r B, S::Err> + 'a,
  S: Subscribable<'a>,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribable = S::Unsubscribable;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribable
  where
    N: 'a + Fn(&Self::Item) -> OState<Self::Err>,
  {
    let func = self.func;
    self.source.subscribe_return_state(move |v| match func(v) {
      Ok(v) => next(&v),
      Err(e) => OState::Err(e),
    })
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Map, MapWithErr},
    prelude::*,
  };
  use std::cell::Cell;

  #[test]
  fn primitive_type() {
    let i = Cell::new(0);
    let subject = Subject::<'_, i32, ()>::new();
    subject.clone().map(|v| v * 2).subscribe(|v| i.set(*v));
    subject.next(&100);
    assert_eq!(i.get(), 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let i = Cell::new(0);
    let subject = Subject::<'_, i32, ()>::new();
    subject
      .clone()
      .map_return_ref(|v: &i32| v)
      .subscribe(|v| i.set(*v));
    subject.next(&100);
    assert_eq!(i.get(), 100);
  }

  #[test]
  fn unsubscribe() {
    let i = Cell::new(0);
    let subject = Subject::<'_, i32, ()>::new();
    subject
      .clone()
      .map_return_ref(|v: &i32| v)
      .subscribe(|v| i.set(*v))
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

    subject.next(&1);
  }

  #[test]
  #[should_panic]
  fn map_return_ref_with_err() {
    let subject = Subject::new();

    subject
      .clone()
      .map_return_ref_with_err(|_| Err("should panic "))
      .subscribe(|_: &i32| {})
      .on_error(|err| panic!(*err));

    subject.next(&1);
  }
}
