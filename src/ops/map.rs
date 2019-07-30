use crate::prelude::*;
use std::sync::Arc;

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<T> {
  fn map<B, F>(self, f: F) -> MapOp<Self, RxFnWrapper<F>>
  where
    Self: Sized,
    F: Fn(&T) -> B,
  {
    MapOp {
      source: self,
      func: RxFnWrapper::new(f),
    }
  }

  /// A version of map extension which return reference, and furthermore，return
  /// type and input item has same lifetime.
  fn map_return_ref<B, F>(self, f: F) -> MapReturnRefOp<Self, RxFnWrapper<F>>
  where
    Self: Sized,
    F: for<'r> Fn(&'r T) -> &'r B,
  {
    MapReturnRefOp {
      source: self,
      func: RxFnWrapper::new(f),
    }
  }
}

pub trait MapWithErr<T> {
  type Err;
  fn map_with_err<B, F>(self, f: F) -> MapWithErrOp<Self, RxFnWrapper<F>>
  where
    Self: Sized,
    F: Fn(&T) -> Result<B, Self::Err>,
  {
    MapWithErrOp {
      source: self,
      func: RxFnWrapper::new(f),
    }
  }

  /// A version of map_with_err extension which return reference, and
  /// furthermore， return item and input item has same lifetime.
  fn map_return_ref_with_err<B, F>(
    self,
    f: F,
  ) -> MapReturnRefWithErrOp<Self, RxFnWrapper<F>>
  where
    Self: Sized,
    F: for<'r> Fn(&'r T) -> Result<&'r B, Self::Err>,
  {
    MapReturnRefWithErrOp {
      source: self,
      func: RxFnWrapper::new(f),
    }
  }
}

impl<'a, O> Map<O::Item> for O where O: ImplSubscribable {}

impl<'a, O> MapWithErr<O::Item> for O
where
  O: ImplSubscribable,
{
  type Err = O::Err;
}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

impl<S, B, M> ImplSubscribable for MapOp<S, M>
where
  M: RxFn(&S::Item) -> B + Send + Sync + 'static,
  S: ImplSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> RxReturn<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.subscribe_return_state(
      move |v| next(&map.call((v,))),
      error,
      complete,
    )
  }
}

impl<S, B, M> Multicast for MapOp<S, M>
where
  S: Multicast,
  M: RxFn(&S::Item) -> B + Send + Sync + 'static,
{
  type Output = MapOp<S::Output, Arc<M>>;
  fn multicast(self) -> Self::Output {
    MapOp {
      source: self.source.multicast(),
      func: Arc::new(self.func),
    }
  }
}

impl<S, B, M> Fork for MapOp<S, Arc<M>>
where
  S: Fork,
  M: RxFn(&S::Item) -> B + Send + Sync + 'static,
{
  type Output = MapOp<S::Output, Arc<M>>;
  fn fork(&self) -> Self::Output {
    MapOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

pub struct MapWithErrOp<S, M> {
  source: S,
  func: M,
}

impl<'a, S, B, M> ImplSubscribable for MapWithErrOp<S, M>
where
  M: RxFn(&S::Item) -> Result<B, S::Err> + Send + Sync + 'static,
  S: ImplSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> RxReturn<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + Sync + 'static>,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.subscribe_return_state(
      move |v| match map.call((v,)) {
        Ok(v) => next(&v),
        Err(e) => RxReturn::Err(e),
      },
      error,
      complete,
    )
  }
}

impl<S, B, M> Multicast for MapWithErrOp<S, M>
where
  S: Multicast,
  M: RxFn(&S::Item) -> Result<B, S::Err> + Send + Sync + 'static,
{
  type Output = MapWithErrOp<S::Output, Arc<M>>;
  fn multicast(self) -> Self::Output {
    MapWithErrOp {
      source: self.source.multicast(),
      func: Arc::new(self.func),
    }
  }
}

impl<S, B, M> Fork for MapWithErrOp<S, Arc<M>>
where
  S: Fork,
  M: RxFn(&S::Item) -> Result<B, S::Err> + Send + Sync + 'static,
{
  type Output = MapWithErrOp<S::Output, Arc<M>>;
  fn fork(&self) -> Self::Output {
    MapWithErrOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

impl<S, B, M> ImplSubscribable for MapReturnRefOp<S, M>
where
  M: for<'r> RxFn(&'r S::Item) -> &'r B + Send + Sync + 'static,
  S: ImplSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> RxReturn<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.subscribe_return_state(
      move |v| next(&map.call((v,))),
      error,
      complete,
    )
  }
}

impl<S, B, M> Multicast for MapReturnRefOp<S, M>
where
  S: Multicast,
  M: for<'r> RxFn(&'r S::Item) -> &'r B + Send + Sync + 'static,
{
  type Output = MapReturnRefOp<S::Output, Arc<M>>;
  fn multicast(self) -> Self::Output {
    MapReturnRefOp {
      source: self.source.multicast(),
      func: Arc::new(self.func),
    }
  }
}

impl<S, B, M> Fork for MapReturnRefOp<S, Arc<M>>
where
  S: Fork,
  M: for<'r> RxFn(&'r S::Item) -> &'r B + Send + Sync + 'static,
{
  type Output = MapReturnRefOp<S::Output, Arc<M>>;
  fn fork(&self) -> Self::Output {
    MapReturnRefOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

pub struct MapReturnRefWithErrOp<S, M> {
  source: S,
  func: M,
}

impl<S, B, M> ImplSubscribable for MapReturnRefWithErrOp<S, M>
where
  M: for<'r> RxFn(&'r S::Item) -> Result<&'r B, S::Err> + Send + Sync + 'static,
  S: ImplSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> RxReturn<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.subscribe_return_state(
      move |v| match map.call((v,)) {
        Ok(v) => next(&v),
        Err(e) => RxReturn::Err(e),
      },
      error,
      complete,
    )
  }
}

impl<S, B, M> Multicast for MapReturnRefWithErrOp<S, M>
where
  S: Multicast,
  M: for<'r> RxFn(&'r S::Item) -> Result<&'r B, S::Err> + Send + Sync + 'static,
{
  type Output = MapReturnRefWithErrOp<S::Output, Arc<M>>;
  fn multicast(self) -> Self::Output {
    MapReturnRefWithErrOp {
      source: self.source.multicast(),
      func: Arc::new(self.func),
    }
  }
}

impl<S, B, M> Fork for MapReturnRefWithErrOp<S, Arc<M>>
where
  S: Fork,
  M: for<'r> RxFn(&'r S::Item) -> Result<&'r B, S::Err> + Send + Sync + 'static,
{
  type Output = MapReturnRefWithErrOp<S::Output, Arc<M>>;
  fn fork(&self) -> Self::Output {
    MapReturnRefWithErrOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

#[cfg(test)]
mod test {
  use crate::{
    ops::{Map, MapWithErr},
    prelude::*,
  };
  use std::sync::{Arc, Mutex};

  #[test]
  fn primitive_type() {
    let i = Arc::new(Mutex::new(0));
    let c_i = i.clone();
    observable::from_range(100..101)
      .map(|v| v * 2)
      .subscribe(move |v| *i.lock().unwrap() += *v);
    assert_eq!(*c_i.lock().unwrap(), 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let i = Arc::new(Mutex::new(0));
    let c_i = i.clone();

    observable::of(100)
      .map_return_ref(|v: &i32| v)
      .subscribe(move |v| *i.lock().unwrap() += *v);
    assert_eq!(*c_i.lock().unwrap(), 100);
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
    // type to type can fork
    let m = observable::from_range(0..100).map(|v| *v);
    m.multicast()
      .fork()
      .map(|v| *v)
      .multicast()
      .fork()
      .subscribe(|_| {});

    // ref to ref can fork
    let m = observable::from_range(0..100).map_return_ref(|v| v);
    m.multicast()
      .fork()
      .map_return_ref(|v| v)
      .multicast()
      .fork()
      .subscribe(|_| {});
    // type to type with error can fork
    let m = observable::from_range(0..100).map_with_err(|v| Ok(*v));
    m.multicast()
      .fork()
      .map_with_err(|v| Ok(*v))
      .multicast()
      .fork()
      .subscribe(|_| {});
    // ref to ref with error can fork
    let m = observable::from_range(0..100).map_return_ref_with_err(|v| Ok(v));
    m.multicast()
      .fork()
      .map_return_ref_with_err(|v| Ok(v))
      .multicast()
      .fork()
      .subscribe(|_| {});
  }
}
