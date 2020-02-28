use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

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
  fn filter_map<F, SourceItem, Item>(self, f: F) -> FilterMapOp<Self, F>
  where
    F: FnMut(SourceItem) -> Option<Item>,
  {
    FilterMapOp { source: self, f }
  }
}

impl<T> FilterMap for T {}

#[derive(Clone)]
pub struct FilterMapOp<S, F> {
  source: S,
  f: F,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.source.actual_subscribe(Subscriber {
      observer: FilterMapObserver {
        down_observer: subscriber.observer,
        f: self.f,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<'a, Item, S, F> Observable for FilterMapOp<S, F>
where
  S: Observable,
  F: FnMut(S::Item) -> Option<Item>,
{
  type Item = Item;
  type Err = S::Err;
}

impl<'a, Item, S, F> LocalObservable<'a> for FilterMapOp<S, F>
where
  S: LocalObservable<'a>,
  F: FnMut(S::Item) -> Option<Item> + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<Item, S, F> SharedObservable for FilterMapOp<S, F>
where
  S: SharedObservable,
  F: FnMut(S::Item) -> Option<Item> + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct FilterMapObserver<O, F> {
  down_observer: O,
  f: F,
}

impl<O, F, Item, Err, OutputItem> Observer<Item, Err>
  for FilterMapObserver<O, F>
where
  O: Observer<OutputItem, Err>,
  F: FnMut(Item) -> Option<OutputItem>,
{
  fn next(&mut self, value: Item) {
    if let Some(v) = (self.f)(value) {
      self.down_observer.next(v)
    }
  }
  error_proxy_impl!(Err, down_observer);
  complete_proxy_impl!(down_observer);
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
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn filter_map_return_ref() {
    observable::of(&1)
      .filter_map(Some)
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }
  #[test]
  fn filter_map_mut_ref() {
    let mut subject = Subject::new().mut_ref_item();
    subject
      .clone()
      .filter_map::<fn(&mut i32) -> Option<&mut i32>, _, _>(|v| Some(v))
      .subscribe(|_: &mut i32| {});

    subject.next(&mut 1);
  }
}
