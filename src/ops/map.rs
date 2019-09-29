use crate::prelude::*;

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

impl<Item, Err, Sub, U, S, B, M> RawSubscribable<Item, Err, Subscriber<Sub, U>>
  for MapOp<S, M>
where
  S: RawSubscribable<B, Err, Subscriber<MapSubscribe<Sub, M>, U>>,
  M: FnMut(&Item) -> B,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<Sub, U>) -> Self::Unsub {
    let map = self.func;
    self.source.raw_subscribe(Subscriber {
      subscribe: MapSubscribe {
        subscribe: subscriber.subscribe,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct MapSubscribe<S, M> {
  subscribe: S,
  map: M,
}

impl<Item, Err, S, M, B> Subscribe<Item, Err> for MapSubscribe<S, M>
where
  S: Subscribe<B, Err>,
  M: FnMut(&Item) -> B,
{
  fn on_next(&mut self, value: &Item) {
    self.subscribe.on_next(&(self.map)(value))
  }

  #[inline(always)]
  fn on_error(&mut self, err: &Err) { self.subscribe.on_error(err); }

  #[inline(always)]
  fn on_complete(&mut self) { self.subscribe.on_complete(); }
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
      subscribe: self.subscribe.to_shared(),
      map: self.map,
    }
  }
}

impl<S, M> IntoShared for MapOp<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapOp<S::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapOp {
      source: self.source.to_shared(),
      func: self.func,
    }
  }
}

pub struct MapReturnRefOp<S, M> {
  source: S,
  func: M,
}

impl<Item, Err, Sub, U, S, B, M> RawSubscribable<Item, Err, Subscriber<Sub, U>>
  for MapReturnRefOp<S, M>
where
  S: RawSubscribable<B, Err, Subscriber<MapReturnRefSubscribe<Sub, M>, U>>,
  M: for<'r> FnMut(&'r Item) -> &'r B,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<Sub, U>) -> Self::Unsub {
    let map = self.func;
    self.source.raw_subscribe(Subscriber {
      subscribe: MapReturnRefSubscribe {
        subscribe: subscriber.subscribe,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct MapReturnRefSubscribe<S, M> {
  subscribe: S,
  map: M,
}

impl<Item, Err, S, M, B> Subscribe<Item, Err> for MapReturnRefSubscribe<S, M>
where
  S: Subscribe<B, Err>,
  M: for<'r> FnMut(&'r Item) -> &'r B,
{
  fn on_next(&mut self, value: &Item) {
    self.subscribe.on_next(&(self.map)(value))
  }

  #[inline(always)]
  fn on_error(&mut self, err: &Err) { self.subscribe.on_error(err); }

  #[inline(always)]
  fn on_complete(&mut self) { self.subscribe.on_complete(); }
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

impl<S, M> IntoShared for MapReturnRefSubscribe<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapReturnRefSubscribe<S::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapReturnRefSubscribe {
      subscribe: self.subscribe.to_shared(),
      map: self.map,
    }
  }
}

impl<S, M> IntoShared for MapReturnRefOp<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapReturnRefOp<S::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapReturnRefOp {
      source: self.source.to_shared(),
      func: self.func,
    }
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
