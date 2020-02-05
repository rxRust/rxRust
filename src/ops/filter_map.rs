use crate::observer::{
  observer_complete_proxy_impl, observer_error_proxy_impl,
};
use crate::ops::SharedOp;
use crate::prelude::*;
use std::marker::PhantomData;

/// `FilterMap` operator applies both `Filter` and `Map`.
pub trait FilterMap
where
  Self: Sized,
{
  /// The closure must return an Option<T>. filter_map creates an iterator which
  /// calls this closure on each element. If the closure returns Some(element),
  /// then that element is returned. If the closure returns None, it will try
  /// again, and call the closure on the next element, seeing if it will return
  /// Some.
  ///
  /// Why filter_map and not just filter and map? The key is in this part:
  ///
  /// If the closure returns Some(element), then that element is returned.
  ///
  /// In other words, it removes the Option<T> layer automatically. If your
  /// mapping is already returning an Option<T> and you want to skip over Nones,
  /// then filter_map is much, much nicer to use.
  ///
  /// # Examples
  ///
  /// ```
  ///  # use rxrust::prelude::*;
  ///  # use rxrust::ops::FilterMap;
  ///  let mut res: Vec<i32> = vec![];
  ///   observable::from_iter(["1", "lol", "3", "NaN", "5"].iter())
  ///   .filter_map(|s: &&str| s.parse().ok())
  ///   .subscribe(|v| res.push(v));
  ///
  /// assert_eq!(res, [1, 3, 5]);
  /// ```
  ///
  fn filter_map<F, SourceItem, Item>(
    self,
    f: F,
  ) -> FilterMapOp<Self, F, SourceItem>
  where
    F: FnMut(SourceItem) -> Option<Item>,
  {
    FilterMapOp {
      source: self,
      f,
      _p: PhantomData,
    }
  }
}

impl<T> FilterMap for T {}

pub struct FilterMapOp<S, F, I> {
  source: S,
  f: F,
  _p: PhantomData<I>,
}

impl<Item, SourceItem, S, F, O, U> Observable<O, U>
  for FilterMapOp<S, F, SourceItem>
where
  U: SubscriptionLike,
  S: Observable<FilterMapObserver<O, F>, U>,
  F: FnMut(SourceItem) -> Option<Item>,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: FilterMapObserver {
        down_observer: subscriber.observer,
        f: self.f,
      },
      subscription: subscriber.subscription,
    })
  }
}

unsafe impl<S, F, I> Send for FilterMapOp<S, F, I>
where
  S: Send,
  F: Send,
{
}

unsafe impl<S, F, I> Sync for FilterMapOp<S, F, I>
where
  S: Sync,
  F: Sync,
{
}

impl<S, F, I> Fork for FilterMapOp<S, F, I>
where
  S: Fork,
  F: Clone,
{
  type Output = FilterMapOp<S::Output, F, I>;
  fn fork(&self) -> Self::Output {
    FilterMapOp {
      source: self.source.fork(),
      f: self.f.clone(),
      _p: PhantomData,
    }
  }
}
impl<S, F, I> IntoShared for FilterMapOp<S, F, I>
where
  S: IntoShared,
  F: Send + Sync + 'static,
  I: 'static,
{
  type Shared = SharedOp<FilterMapOp<S::Shared, F, I>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(FilterMapOp {
      source: self.source.to_shared(),
      f: self.f,
      _p: PhantomData,
    })
  }
}

pub struct FilterMapObserver<O, F> {
  down_observer: O,
  f: F,
}
impl<O, F, Item, OutputItem> ObserverNext<Item> for FilterMapObserver<O, F>
where
  O: ObserverNext<OutputItem>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  fn next(&mut self, value: Item) {
    if let Some(v) = (self.f)(value) {
      self.down_observer.next(v)
    }
  }
}

observer_error_proxy_impl!(FilterMapObserver<O, F>, O, down_observer, <O, F>);
observer_complete_proxy_impl!(
  FilterMapObserver<O, F>, O, down_observer, <O, F>);

impl<O, F> IntoShared for FilterMapObserver<O, F>
where
  O: IntoShared,
  F: Send + Sync + 'static,
{
  type Shared = FilterMapObserver<O::Shared, F>;
  fn to_shared(self) -> Self::Shared {
    FilterMapObserver {
      down_observer: self.down_observer.to_shared(),
      f: self.f,
    }
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::FilterMap, prelude::*};

  #[test]
  fn map_types_mixed() {
    let mut i = 0;
    observable::from_iter(vec!['a', 'b', 'c'])
      .filter_map(|_v| Some(1))
      .subscribe(|v| i += v);
    assert_eq!(i, 3);
  }

  #[test]
  fn filter_map_shared_and_fork() {
    observable::of(1)
      .filter_map(|_| Some("str"))
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn filter_map_return_ref() {
    observable::of(&1)
      .filter_map(Some)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
  #[test]
  fn filter_map_mut_ref() {
    let mut subject = Subject::local();
    subject
      .fork()
      .filter_map::<fn(&mut i32) -> Option<&mut i32>, _, _>(|v| Some(v))
      .subscribe(|_: &mut i32| {});

    subject.next(&mut 1);
  }
}
