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

impl<'a, O> Map<O::Item> for O where O: RawSubscribable {}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

macro_rules! map_subscribe {
  ($subscribe: ident, $map: ident) => {
    RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| match v {
      RxValue::Next(nv) => $subscribe.call((RxValue::Next(&$map.call((nv,))),)),
      RxValue::Err(err) => $subscribe.call((RxValue::Err(err),)),
      RxValue::Complete => $subscribe.call((RxValue::Complete,)),
    })
  };
}

impl<S, B, M> RawSubscribable for MapOp<S, M>
where
  M: RxFn(&S::Item) -> B + Send + Sync + 'static,
  S: RawSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.raw_subscribe(map_subscribe!(subscribe, map))
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

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

impl<S, B, M> RawSubscribable for MapReturnRefOp<S, M>
where
  M: for<'r> RxFn(&'r S::Item) -> &'r B + Send + Sync + 'static,
  S: RawSubscribable,
{
  type Item = B;
  type Err = S::Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let map = self.func;
    self.source.raw_subscribe(map_subscribe!(subscribe, map))
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

#[cfg(test)]
mod test {
  use crate::{ops::Map, prelude::*};
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
  }
}
