use crate::observer::{
  observer_complete_proxy_impl, observer_error_proxy_impl,
};
use crate::prelude::*;
use ops::SharedOp;
use std::marker::PhantomData;

pub trait Map<T> {
  /// Creates a new stream which calls a closure on each element and uses
  /// its return as the value.
  fn map<B, F>(self, f: F) -> MapOp<Self, F, B>
  where
    Self: Sized,
    F: Fn(B) -> T,
  {
    MapOp {
      source: self,
      func: f,
      _p: PhantomData,
    }
  }
}

impl<O, Item> Map<Item> for O {}

pub struct MapOp<S, M, B> {
  source: S,
  func: M,
  _p: PhantomData<B>,
}

impl<Item, B, O, U, S, M> Observable<O, U> for MapOp<S, M, B>
where
  S: Observable<MapObserver<O, M>, U>,
  M: FnMut(B) -> Item,
  U: SubscriptionLike,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let map = self.func;
    self.source.actual_subscribe(Subscriber {
      observer: MapObserver {
        observer: subscriber.observer,
        map,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct MapObserver<S, M> {
  observer: S,
  map: M,
}

impl<Item, S, M, B> ObserverNext<Item> for MapObserver<S, M>
where
  S: ObserverNext<B>,
  M: FnMut(Item) -> B,
{
  fn next(&mut self, value: Item) { self.observer.next((self.map)(value)) }
}

observer_complete_proxy_impl!(MapObserver<O, M>, O, observer, <O, M>);
observer_error_proxy_impl!(MapObserver<O, M>, O, observer, <O, M, Err>, Err);

impl<S, M, B> Fork for MapOp<S, M, B>
where
  S: Fork,
  M: Clone,
{
  type Output = MapOp<S::Output, M, B>;
  fn fork(&self) -> Self::Output {
    MapOp {
      source: self.source.fork(),
      func: self.func.clone(),
      _p: PhantomData,
    }
  }
}

impl<S, M> IntoShared for MapObserver<S, M>
where
  S: IntoShared,
  M: Send + Sync + 'static,
{
  type Shared = MapObserver<S::Shared, M>;
  fn to_shared(self) -> Self::Shared {
    MapObserver {
      observer: self.observer.to_shared(),
      map: self.map,
    }
  }
}

impl<S, M, B> IntoShared for MapOp<S, M, B>
where
  S: IntoShared,
  M: Send + Sync + 'static,
  B: Send + Sync + 'static,
{
  type Shared = SharedOp<MapOp<S::Shared, M, B>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(MapOp {
      source: self.source.to_shared(),
      func: self.func,
      _p: PhantomData,
    })
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Map, prelude::*};

  #[test]
  fn primitive_type() {
    let mut i = 0;
    observable::from_iter(100..101)
      .map(|v| v * 2)
      .subscribe(|v| i += v);
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;

    observable::of(100).map(|v| v).subscribe(|v| i += v);
    assert_eq!(i, 100);
  }

  #[test]
  fn fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).map(|v| v);
    m.fork()
      .map(|v| v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});

    // type mapped to other type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).map(|_v| 1);
    m.fork()
      .map(|v| v as f32)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});

    // ref to ref can fork
    let m = observable::of(&1).map(|v| v);
    m.fork()
      .map(|v| v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .map(|_v| 1)
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }
}
