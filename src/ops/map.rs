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

#[inline]
fn subscribe_source<'a, S, M, B>(
  source: S,
  map: M,
  next: impl Fn(&B) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Box<dyn Subscription + 'a>
where
  M: Fn(&S::Item) -> B + 'a,
  S: ImplSubscribable<'a>,
{
  source.subscribe_return_state(move |v| next(&map(v)), error, complete)
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

#[inline]
fn subscribe_source_with_err<'a, S, M, B>(
  source: S,
  map: M,
  next: impl Fn(&B) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Box<dyn Subscription + 'a>
where
  M: Fn(&S::Item) -> Result<B, S::Err> + 'a,
  S: ImplSubscribable<'a>,
{
  source.subscribe_return_state(
    move |v| match map(v) {
      Ok(v) => next(&v),
      Err(e) => OState::Err(e),
    },
    error,
    complete,
  )
}
impl<'a, O> Map<'a, O::Item> for O where O: ImplSubscribable<'a> {}

impl<'a, O> MapWithErr<'a, O::Item> for O
where
  O: ImplSubscribable<'a>,
{
  type Err = O::Err;
}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> ImplSubscribable<'a> for MapOp<S, M>
where
  M: Fn(&S::Item) -> B + 'a,
  S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source(self.source, self.func, next, error, complete)
  }
}

impl<'a, S, B, M> ImplSubscribable<'a> for &'a MapOp<S, M>
where
  M: Fn(&<&'a S as ImplSubscribable<'a>>::Item) -> B + 'a,
  &'a S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = <&'a S as ImplSubscribable<'a>>::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source(&self.source, &self.func, next, error, complete)
  }
}

pub struct MapWithErrOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> ImplSubscribable<'a> for MapWithErrOp<S, M>
where
  M: Fn(&S::Item) -> Result<B, S::Err> + 'a,
  S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_with_err(self.source, self.func, next, error, complete)
  }
}

impl<'a, S, B, M> ImplSubscribable<'a> for &'a MapWithErrOp<S, M>
where
  M: Fn(
      &<&'a S as ImplSubscribable<'a>>::Item,
    ) -> Result<B, <&'a S as ImplSubscribable<'a>>::Err>
    + 'a,
  &'a S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = <&'a S as ImplSubscribable<'a>>::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_with_err(&self.source, &self.func, next, error, complete)
  }
}

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

#[inline]
fn subscribe_source_ref<'a, S, M, B>(
  source: S,
  map: M,
  next: impl Fn(&B) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Box<dyn Subscription + 'a>
where
  M: for<'r> Fn(&'r S::Item) -> &'r B + 'a,
  S: ImplSubscribable<'a>,
{
  source.subscribe_return_state(move |v| next(&map(v)), error, complete)
}

impl<'a, S, B, M> ImplSubscribable<'a> for MapReturnRefOp<S, M>
where
  M: for<'r> Fn(&'r S::Item) -> &'r B + 'a,
  S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_ref(self.source, self.func, next, error, complete)
  }
}

impl<'a, S, B, M> ImplSubscribable<'a> for &'a MapReturnRefOp<S, M>
where
  M: for<'r> Fn(&'r <&'a S as ImplSubscribable<'a>>::Item) -> &'r B + 'a,
  &'a S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = <&'a S as ImplSubscribable<'a>>::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_ref(&self.source, &self.func, next, error, complete)
  }
}

pub struct MapReturnRefWithErrOp<S, M> {
  source: S,
  func: M,
}

#[inline]
fn subscribe_source_with_err_ref<'a, S, M, B>(
  source: S,
  map: M,
  next: impl Fn(&B) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Box<dyn Subscription + 'a>
where
  M: for<'r> Fn(&'r S::Item) -> Result<&'r B, S::Err> + 'a,
  S: ImplSubscribable<'a>,
{
  source.subscribe_return_state(
    move |v| match map(v) {
      Ok(v) => next(&v),
      Err(e) => OState::Err(e),
    },
    error,
    complete,
  )
}

impl<'a, S, B, M> ImplSubscribable<'a> for MapReturnRefWithErrOp<S, M>
where
  M: for<'r> Fn(&'r S::Item) -> Result<&'r B, S::Err> + 'a,
  S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_with_err_ref(self.source, self.func, next, error, complete)
  }
}

impl<'a, S, B, M> ImplSubscribable<'a> for &'a MapReturnRefWithErrOp<S, M>
where
  M: for<'r> Fn(
      &'r <&'a S as ImplSubscribable<'a>>::Item,
    ) -> Result<&'r B, <&'a S as ImplSubscribable<'a>>::Err>
    + 'a,
  &'a S: ImplSubscribable<'a>,
{
  type Item = B;
  type Err = <&'a S as ImplSubscribable<'a>>::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    subscribe_source_with_err_ref(
      &self.source,
      &self.func,
      next,
      error,
      complete,
    )
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
    observable::from_iter(100..101)
      .map(|v| v * 2)
      .subscribe(|v| i.set(*v));
    assert_eq!(i.get(), 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let i = Cell::new(0);

    observable::of(100)
      .map_return_ref(|v: &i32| v)
      .subscribe(|v| i.set(*v));
    assert_eq!(i.get(), 100);
  }

  #[test]
  #[should_panic]
  fn with_err() {
    let subject = Subject::new();

    subject
      .clone()
      .map_with_err(|_| Err("should panic "))
      .subscribe_err(|_: &i32| {}, |err| panic!(*err));

    subject.next(&1);
  }

  #[test]
  #[should_panic]
  fn map_return_ref_with_err() {
    let subject = Subject::new();

    subject
      .clone()
      .map_return_ref_with_err(|_| Err("should panic "))
      .subscribe_err(|_: &i32| {}, |err| panic!(*err));

    subject.next(&1);
  }

  #[test]
  fn fork() {
    use crate::ops::Fork;
    // type to type can fork
    let m = observable::from_iter(0..100).map(|v| *v);
    m.fork().map(|v| *v).fork().subscribe(|_| {});
    // ref to ref can fork
    let m = observable::from_iter(0..100).map_return_ref(|v| v);
    m.fork().map_return_ref(|v| v).fork().subscribe(|_| {});
    // type to type with error can fork
    let m = observable::from_iter(0..100).map_with_err(|v| Ok(*v));
    m.fork().map_with_err(|v| Ok(*v)).fork().subscribe(|_| {});
    // ref to ref with error can fork
    let m = observable::from_iter(0..100).map_return_ref_with_err(|v| Ok(v));
    m.fork()
      .map_return_ref_with_err(|v| Ok(v))
      .fork()
      .subscribe(|_| {});
  }
}
