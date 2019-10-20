use crate::prelude::*;
use ops::SharedOp;

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<T> {
  fn map<B, F>(self, f: F) -> MapOp<Self, F>
  where
    Self: Sized,
    F: Fn(&T) -> B,
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
    F: for<'r> Fn(&'r T) -> &'r B,
  {
    MapReturnRefOp {
      source: self,
      func: f,
    }
  }
}

impl<O, Item> Map<Item> for O {}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

impl<Item, Err, O, U, S, B, M> RawSubscribable<Item, Err, Subscriber<O, U>>
  for MapOp<S, M>
where
  S: RawSubscribable<B, Err, Subscriber<MapSubscribe<O, M>, U>>,
  M: FnMut(&Item) -> B,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let map = self.func;
    self.source.raw_subscribe(Subscriber {
      observer: MapSubscribe {
        observer: subscriber.observer,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct MapSubscribe<S, M> {
  observer: S,
  map: M,
}

impl<Item, Err, S, M, B> Observer<Item, Err> for MapSubscribe<S, M>
where
  S: Observer<B, Err>,
  M: FnMut(&Item) -> B,
{
  fn next(&mut self, value: &Item) { self.observer.next(&(self.map)(value)) }

  #[inline(always)]
  fn error(&mut self, err: &Err) { self.observer.error(err); }

  #[inline(always)]
  fn complete(&mut self) { self.observer.complete(); }
}

impl<S, M> Fork for MapOp<S, M>
where
  S: Fork,
  M: Clone,
{
  type Output = MapOp<S::Output, M>;
  fn fork(&self) -> Self::Output {
    MapOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

impl<S, M> IntoShared for MapSubscribe<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapSubscribe<S::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapSubscribe {
      observer: self.observer.to_shared(),
      map: self.map,
    }
  }
}

impl<S, M> IntoShared for MapOp<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = SharedOp<MapOp<S::Shared, M>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(MapOp {
      source: self.source.to_shared(),
      func: self.func,
    })
  }
}

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

impl<Item, Err, O, U, S, B, M> RawSubscribable<Item, Err, Subscriber<O, U>>
  for MapReturnRefOp<S, M>
where
  S: RawSubscribable<B, Err, Subscriber<MapReturnRefSubscribe<O, M>, U>>,
  M: for<'r> FnMut(&'r Item) -> &'r B,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let map = self.func;
    self.source.raw_subscribe(Subscriber {
      observer: MapReturnRefSubscribe {
        observer: subscriber.observer,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct MapReturnRefSubscribe<O, M> {
  observer: O,
  map: M,
}

impl<Item, Err, O, M, B> Observer<Item, Err> for MapReturnRefSubscribe<O, M>
where
  O: Observer<B, Err>,
  M: for<'r> FnMut(&'r Item) -> &'r B,
{
  fn next(&mut self, value: &Item) { self.observer.next(&(self.map)(value)) }

  #[inline(always)]
  fn error(&mut self, err: &Err) { self.observer.error(err); }

  #[inline(always)]
  fn complete(&mut self) { self.observer.complete(); }
}

impl<S, M> Fork for MapReturnRefOp<S, M>
where
  S: Fork,
  M: Clone,
{
  type Output = MapReturnRefOp<S::Output, M>;
  fn fork(&self) -> Self::Output {
    MapReturnRefOp {
      source: self.source.fork(),
      func: self.func.clone(),
    }
  }
}

impl<O, M> IntoShared for MapReturnRefSubscribe<O, M>
where
  O: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapReturnRefSubscribe<O::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapReturnRefSubscribe {
      observer: self.observer.to_shared(),
      map: self.map,
    }
  }
}

impl<S, M> IntoShared for MapReturnRefOp<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = SharedOp<MapReturnRefOp<S::Shared, M>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(MapReturnRefOp {
      source: self.source.to_shared(),
      func: self.func,
    })
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Map, prelude::*};

  #[test]
  fn primitive_type() {
    let mut i = 0;
    observable::from_iter!(100..101)
      .map(|v| v * 2)
      .subscribe(|v| i += *v);
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of!(100)
      .map_return_ref(|v| v)
      .subscribe(|v| i += *v);
    assert_eq!(i, 100);
  }

  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter!(0..100).map(|v| *v);
    m.fork()
      .map(|v| *v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});

    // ref to ref can fork
    let m = observable::from_iter!(0..100).map_return_ref(|v| v);
    m.fork()
      .map_return_ref(|v| v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
