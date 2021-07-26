#![macro_use]
mod trivial;
pub use trivial::*;

mod from_iter;
pub use from_iter::{from_iter, repeat};

pub mod of;
pub use of::{of, of_fn, of_option, of_result};

pub(crate) mod from_future;
pub use from_future::{from_future, from_future_result};

pub mod interval;
pub use interval::{interval, interval_at};

pub(crate) mod connectable_observable;
pub use connectable_observable::{
  ConnectableObservable, LocalConnectableObservable,
  SharedConnectableObservable,
};

mod observable_block_all;
#[cfg(test)]
pub use observable_block_all::*;

mod observable_block;
#[cfg(test)]
pub use observable_block::*;

mod base;
pub use base::*;

pub mod from_fn;
pub use from_fn::*;

mod observable_all;
pub use observable_all::*;
mod observable_err;
pub use observable_err::*;
mod observable_next;
pub use observable_next::*;
mod observable_comp;

mod defer;

pub use defer::*;

use crate::prelude::*;
pub use observable_comp::*;

use crate::ops::default_if_empty::DefaultIfEmptyOp;
use ops::{
  box_it::{BoxOp, IntoBox},
  contains::ContainsOp,
  debounce::DebounceOp,
  delay::DelayOp,
  distinct::DistinctOp,
  filter::FilterOp,
  filter_map::FilterMapOp,
  finalize::FinalizeOp,
  flatten::FlattenOp,
  last::LastOp,
  map::MapOp,
  map_to::MapToOp,
  merge::MergeOp,
  observe_on::ObserveOnOp,
  ref_count::{RefCount, RefCountCreator},
  sample::SampleOp,
  scan::ScanOp,
  skip::SkipOp,
  skip_last::SkipLastOp,
  skip_while::SkipWhileOp,
  subscribe_on::SubscribeOnOP,
  take::TakeOp,
  take_last::TakeLastOp,
  take_until::TakeUntilOp,
  take_while::TakeWhileOp,
  throttle_time::{ThrottleEdge, ThrottleTimeOp},
  zip::ZipOp,
  Accum, AverageOp, CountOp, FlatMapOp, MinMaxOp, ReduceOp, SumOp,
};
use std::ops::{Add, Mul};
use std::time::{Duration, Instant};

type ALLOp<O, F> =
  DefaultIfEmptyOp<TakeOp<FilterOp<MapOp<O, F>, fn(&bool) -> bool>>>;

pub trait Observable: Sized {
  type Item;
  type Err;

  /// emit only the first item emitted by an Observable
  #[inline]
  fn first(self) -> TakeOp<Self> { self.take(1) }

  /// emit only the first item emitted by an Observable
  #[inline]
  fn first_or(self, default: Self::Item) -> DefaultIfEmptyOp<TakeOp<Self>> {
    self.first().default_if_empty(default)
  }

  /// Emit only the last final item emitted by a source observable or a
  /// default item given.
  ///
  /// Completes right after emitting the single item. Emits error when
  /// source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::empty()
  ///   .last_or(1234)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 1234
  /// ```
  #[inline]
  fn last_or(
    self,
    default: Self::Item,
  ) -> DefaultIfEmptyOp<LastOp<Self, Self::Item>> {
    self.last().default_if_empty(default)
  }

  /// Emit only item n (0-indexed) emitted by an Observable
  #[inline]
  fn element_at(self, nth: u32) -> TakeOp<SkipOp<Self>> {
    self.skip(nth).first()
  }

  /// Do not emit any items from an Observable but mirror its termination
  /// notification
  #[inline]
  fn ignore_elements(self) -> FilterOp<Self, fn(&Self::Item) -> bool> {
    fn always_false<Item>(_: &Item) -> bool { false }
    self.filter(always_false as fn(&Self::Item) -> bool)
  }

  /// Determine whether all items emitted by an Observable meet some criteria
  #[inline]
  fn all<F>(self, pred: F) -> ALLOp<Self, F>
  where
    F: Fn(Self::Item) -> bool,
  {
    fn not(b: &bool) -> bool { !b }
    self
      .map(pred)
      .filter(not as fn(&bool) -> bool)
      .first_or(true)
  }

  /// Determine whether an Observable emits a particular item or not
  fn contains(self, target: Self::Item) -> ContainsOp<Self, Self::Item> {
    ContainsOp {
      source: self,
      target,
    }
  }

  /// Emits only last final item emitted by a source observable.
  ///
  /// Completes right after emitting the single last item, or when source
  /// observable completed, being an empty one. Emits error when source
  /// observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..100)
  ///   .last()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 99
  /// ```
  #[inline]
  fn last(self) -> LastOp<Self, Self::Item> {
    LastOp {
      source: self,
      last: None,
    }
  }

  /// Call a function when observable completes, errors or is unsubscribed from.
  #[inline]
  fn finalize<F>(self, f: F) -> FinalizeOp<Self, F>
  where
    F: FnMut(),
  {
    FinalizeOp {
      source: self,
      func: f,
    }
  }

  /// Creates an Observable that combines all the emissions from Observables
  /// that get emitted from an Observable.
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// let mut source = LocalSubject::new();
  /// let numbers = LocalSubject::new();
  /// // create a even stream by filter
  /// let even = numbers.clone().filter((|v| *v % 2 == 0) as fn(&i32) -> bool);
  /// // create an odd stream by filter
  /// let odd = numbers.clone().filter((|v| *v % 2 != 0) as fn(&i32) -> bool);
  ///
  /// // merge odd and even stream again
  /// let out = source.clone().flatten();
  ///
  /// source.next(even);
  /// source.next(odd);
  ///
  /// // attach observers
  /// out.subscribe(|v: i32| println!("{} ", v));
  /// ```
  #[inline]
  fn flatten<Inner, A>(self) -> FlattenOp<Self, Inner>
  where
    Inner: Observable<Item = A, Err = Self::Err>,
  {
    FlattenOp {
      source: self,
      marker: std::marker::PhantomData::<Inner>,
    }
  }

  ///  Applies given function to each item emitted by this Observable, where
  ///  that function returns an Observable that itself emits items. It then
  ///  merges the emissions of these resulting Observables, emitting these
  ///  merged results as its own sequence.
  #[inline]
  fn flat_map<Inner, B, F>(self, f: F) -> FlatMapOp<Self, Inner, F>
  where
    Inner: Observable<Item = B, Err = Self::Err>,
    F: Fn(Self::Item) -> Inner,
  {
    FlattenOp {
      source: MapOp {
        source: self,
        func: f,
      },
      marker: std::marker::PhantomData::<Inner>,
    }
  }

  /// Creates a new stream which calls a closure on each element and uses
  /// its return as the value.
  #[inline]
  fn map<B, F>(self, f: F) -> MapOp<Self, F>
  where
    F: Fn(Self::Item) -> B,
  {
    MapOp {
      source: self,
      func: f,
    }
  }

  /// Maps emissions to a constant value.
  #[inline]
  fn map_to<B>(self, value: B) -> MapToOp<Self, B> {
    MapToOp {
      source: self,
      value,
    }
  }

  /// combine two Observables into one by merging their emissions
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// let numbers = LocalSubject::new();
  /// // create a even stream by filter
  /// let even = numbers.clone().filter(|v| *v % 2 == 0);
  /// // create an odd stream by filter
  /// let odd = numbers.clone().filter(|v| *v % 2 != 0);
  ///
  /// // merge odd and even stream again
  /// let merged = even.merge(odd);
  ///
  /// // attach observers
  /// merged.subscribe(|v: &i32| println!("{} ", v));
  /// ```
  #[inline]
  fn merge<S>(self, o: S) -> MergeOp<Self, S>
  where
    S: Observable<Item = Self::Item, Err = Self::Err>,
  {
    MergeOp {
      source1: self,
      source2: o,
    }
  }

  /// Emit only those items from an Observable that pass a predicate test
  /// # Example
  ///
  /// ```
  /// use rxrust:: prelude::*;
  ///
  /// let mut coll = vec![];
  /// let coll_clone = coll.clone();
  ///
  /// observable::from_iter(0..10)
  ///   .filter(|v| *v % 2 == 0)
  ///   .subscribe(|v| { coll.push(v); });
  ///
  /// // only even numbers received.
  /// assert_eq!(coll, vec![0, 2, 4, 6, 8]);
  /// ```
  #[inline]
  fn filter<F>(self, filter: F) -> FilterOp<Self, F>
  where
    F: Fn(&Self::Item) -> bool,
  {
    FilterOp {
      source: self,
      filter,
    }
  }

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
  ///  let mut res: Vec<i32> = vec![];
  ///   observable::from_iter(["1", "lol", "3", "NaN", "5"].iter())
  ///   .filter_map(|s: &&str| s.parse().ok())
  ///   .subscribe(|v| res.push(v));
  ///
  /// assert_eq!(res, [1, 3, 5]);
  /// ```
  #[inline]
  fn filter_map<F, SourceItem, Item>(self, f: F) -> FilterMapOp<Self, F>
  where
    F: FnMut(SourceItem) -> Option<Item>,
  {
    FilterMapOp { source: self, f }
  }

  /// box an observable to a safety object and convert it to a simple type
  /// `BoxOp`, which only care `Item` and `Err` Observable emitted.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  /// use ops::box_it::LocalBoxOp;
  ///
  /// let mut boxed: LocalBoxOp<'_, i32, ()> = observable::of(1)
  ///   .map(|v| v).box_it();
  ///
  /// // BoxOp can box any observable type
  /// boxed = observable::empty().box_it();
  ///
  /// boxed.subscribe(|_| {});
  /// ```
  #[inline]
  fn box_it<O: IntoBox<Self>>(self) -> BoxOp<O>
  where
    BoxOp<O>: Observable<Item = Self::Item, Err = Self::Err>,
  {
    O::box_it(self)
  }

  /// Ignore the first `count` values emitted by the source Observable.
  ///
  /// `skip` returns an Observable that ignore the first `count` values
  /// emitted by the source Observable. If the source emits fewer than `count`
  /// values then 0 of its values are emitted. After that, it completes,
  /// regardless if the source completes.
  ///
  /// # Example
  /// Ignore the first 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10).skip(5).subscribe(|v| println!("{}", v));

  /// // print logs:
  /// // 6
  /// // 7
  /// // 8
  /// // 9
  /// // 10
  /// ```
  #[inline]
  fn skip(self, count: u32) -> SkipOp<Self> {
    SkipOp {
      source: self,
      count,
    }
  }

  /// Ignore values while result of a callback is true.
  ///
  /// `skip_while` returns an Observable that ignores values while result of an
  /// callback is true emitted by the source Observable.
  ///
  /// # Example
  /// Suppress the first 5 items of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10)
  ///   .skip_while(|v| v < &5)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print logs:
  /// // 5
  /// // 6
  /// // 7
  /// // 8
  /// // 9
  /// ```
  #[inline]
  fn skip_while<F>(self, callback: F) -> SkipWhileOp<Self, F>
  where
    F: FnMut(&Self::Item) -> bool,
  {
    SkipWhileOp {
      source: self,
      callback,
    }
  }

  /// Ignore the last `count` values emitted by the source Observable.
  ///
  /// `skip_last` returns an Observable that ignore the last `count` values
  /// emitted by the source Observable. If the source emits fewer than `count`
  /// values then 0 of its values are emitted.
  /// It will not emit values until source Observable complete.
  ///
  /// # Example
  /// Skip the last 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10)
  ///   .skip_last(5)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print logs:
  /// // 0
  /// // 1
  /// // 2
  /// // 3
  /// // 4
  /// ```
  #[inline]
  fn skip_last(self, count: usize) -> SkipLastOp<Self> {
    SkipLastOp {
      source: self,
      count,
    }
  }

  /// Emits only the first `count` values emitted by the source Observable.
  ///
  /// `take` returns an Observable that emits only the first `count` values
  /// emitted by the source Observable. If the source emits fewer than `count`
  /// values then all of its values are emitted. After that, it completes,
  /// regardless if the source completes.
  ///
  /// # Example
  /// Take the first 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10).take(5).subscribe(|v| println!("{}", v));

  /// // print logs:
  /// // 0
  /// // 1
  /// // 2
  /// // 3
  /// // 4
  /// ```
  ///
  #[inline]
  fn take(self, count: u32) -> TakeOp<Self> {
    TakeOp {
      source: self,
      count,
    }
  }

  /// Emits the values emitted by the source Observable until a `notifier`
  /// Observable emits a value.
  ///
  /// `take_until` subscribes and begins mirroring the source Observable. It
  /// also monitors a second Observable, `notifier` that you provide. If the
  /// `notifier` emits a value, the output Observable stops mirroring the source
  /// Observable and completes. If the `notifier` doesn't emit any value and
  /// completes then `take_until` will pass all values.
  #[inline]
  fn take_until<T>(self, notifier: T) -> TakeUntilOp<Self, T> {
    TakeUntilOp {
      source: self,
      notifier,
    }
  }

  /// Emits values while result of an callback is true.
  ///
  /// `take_while` returns an Observable that emits values while result of an
  /// callback is true emitted by the source Observable.
  /// It will not emit values until source Observable complete.
  ///
  /// # Example
  /// Take the first 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10)
  ///   .take_while(|v| v < &5)
  /// .subscribe(|v| println!("{}", v));

  /// // print logs:
  /// // 0
  /// // 1
  /// // 2
  /// // 3
  /// // 4
  /// ```
  ///
  #[inline]
  fn take_while<F>(self, callback: F) -> TakeWhileOp<Self, F>
  where
    F: FnMut(&Self::Item) -> bool,
  {
    TakeWhileOp {
      source: self,
      callback,
    }
  }

  /// Emits only the last `count` values emitted by the source Observable.
  ///
  /// `take_last` returns an Observable that emits only the last `count` values
  /// emitted by the source Observable. If the source emits fewer than `count`
  /// values then all of its values are emitted.
  /// It will not emit values until source Observable complete.
  ///
  /// # Example
  /// Take the last 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10)
  ///   .take_last(5)
  /// .subscribe(|v| println!("{}", v));

  /// // print logs:
  /// // 5
  /// // 6
  /// // 7
  /// // 8
  /// // 9
  /// ```
  ///
  #[inline]
  fn take_last(self, count: usize) -> TakeLastOp<Self> {
    TakeLastOp {
      source: self,
      count,
    }
  }

  /// Emits item it has most recently emitted since the previous sampling
  ///
  ///
  /// It will emit values when sampling observable complete.
  ///
  /// #Example
  /// Sampling every  5ms of an infinite 1ms interval Observable
  /// ```
  /// use rxrust::prelude::*;
  /// use std::time::Duration;
  /// use futures::executor::LocalPool;
  ///
  /// let mut local_scheduler = LocalPool::new();
  /// let spawner = local_scheduler.spawner();
  /// observable::interval(Duration::from_millis(2), spawner.clone())
  ///   .sample(observable::interval(Duration::from_millis(5), spawner))
  ///   .take(5)
  ///   .subscribe(move |v| println!("{}", v));
  ///
  /// local_scheduler.run();
  /// // print logs:
  /// // 1
  /// // 4
  /// // 6
  /// // 9
  /// // ...
  /// ```
  #[inline]
  fn sample<O>(self, sampling: O) -> SampleOp<Self, O>
  where
    O: Observable,
  {
    SampleOp {
      source: self,
      sampling,
    }
  }

  /// The Scan operator applies a function to the first item emitted by the
  /// source observable and then emits the result of that function as its
  /// own first emission. It also feeds the result of the function back into
  /// the function along with the second item emitted by the source observable
  /// in order to generate its second emission. It continues to feed back its
  /// own subsequent emissions along with the subsequent emissions from the
  /// source Observable in order to create the rest of its sequence.
  ///
  /// Applies a binary operator closure to each item emitted from source
  /// observable and emits successive values.
  ///
  /// Completes when source observable completes.
  /// Emits error when source observable emits it.
  ///
  /// This version starts with an user-specified initial value for when the
  /// binary operator is called with the first item processed.
  ///
  /// # Arguments
  ///
  /// * `initial_value` - An initial value to start the successive accumulations
  ///   from.
  /// * `binary_op` - A closure or function acting as a binary operator.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .scan_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 101
  /// // 102
  /// // 103
  /// // 104
  /// // 105
  /// ```
  #[inline]
  fn scan_initial<OutputItem, BinaryOp>(
    self,
    initial_value: OutputItem,
    binary_op: BinaryOp,
  ) -> ScanOp<Self, BinaryOp, OutputItem>
  where
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
    OutputItem: Clone,
  {
    ScanOp {
      source_observable: self,
      binary_op,
      initial_value,
    }
  }

  /// Works like [`scan_initial`](Observable::scan_initial) but starts with a
  /// value defined by a [`Default`] trait for the first argument `binary_op`
  /// operator operates on.
  ///
  /// # Arguments
  ///
  /// * `binary_op` - A closure or function acting as a binary operator.
  #[inline]
  fn scan<OutputItem, BinaryOp>(
    self,
    binary_op: BinaryOp,
  ) -> ScanOp<Self, BinaryOp, OutputItem>
  where
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
    OutputItem: Default + Clone,
  {
    self.scan_initial(OutputItem::default(), binary_op)
  }

  /// Apply a function to each item emitted by an observable, sequentially,
  /// and emit the final value, after source observable completes.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Arguments
  ///
  /// * `initial` - An initial value to start the successive reduction from.
  /// * `binary_op` - A closure acting as a binary (folding) operator.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .reduce_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 105
  /// ```
  #[inline]
  fn reduce_initial<OutputItem, BinaryOp>(
    self,
    initial: OutputItem,
    binary_op: BinaryOp,
  ) -> ReduceOp<Self, BinaryOp, OutputItem>
  where
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
    OutputItem: Clone,
  {
    // realised as a composition of `scan`, and `last`
    self
      .scan_initial(initial.clone(), binary_op)
      .last_or(initial)
  }

  /// Works like [`reduce_initial`](Observable::reduce_initial) but starts with
  /// a value defined by a [`Default`] trait for the first argument `f`
  /// operator operates on.
  ///
  /// # Arguments
  ///
  /// * `binary_op` - A closure acting as a binary operator.
  #[inline]
  fn reduce<OutputItem, BinaryOp>(
    self,
    binary_op: BinaryOp,
  ) -> DefaultIfEmptyOp<LastOp<ScanOp<Self, BinaryOp, OutputItem>, OutputItem>>
  where
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
    OutputItem: Default + Clone,
  {
    self.reduce_initial(OutputItem::default(), binary_op)
  }

  /// Emits the item from the source observable that had the maximum value.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![3., 4., 7., 5., 6.])
  ///   .max()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 7
  /// ```
  #[inline]
  fn max(self) -> MinMaxOp<Self, Self::Item>
  where
    Self::Item: Clone + Send + PartialOrd<Self::Item>,
  {
    fn get_greater<Item>(i: Option<Item>, v: Item) -> Option<Item>
    where
      Item: Clone + PartialOrd<Item>,
    {
      i.map(|vv| if vv < v { v.clone() } else { vv }).or(Some(v))
    }
    let get_greater_func =
      get_greater as fn(Option<Self::Item>, Self::Item) -> Option<Self::Item>;

    self
      .scan_initial(None, get_greater_func)
      .last()
      // we can safely unwrap, because we will ever get this item
      // once a max value exists and is there.
      .map(|v| v.unwrap())
  }

  /// Emits the item from the source observable that had the minimum value.
  ///
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![3., 4., 7., 5., 6.])
  ///   .min()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 3
  /// ```
  #[inline]
  fn min(self) -> MinMaxOp<Self, Self::Item>
  where
    Self::Item: Clone + Send + PartialOrd<Self::Item>,
  {
    fn get_lesser<Item>(i: Option<Item>, v: Item) -> Option<Item>
    where
      Item: Clone + PartialOrd<Item>,
    {
      i.map(|vv| if vv > v { v.clone() } else { vv }).or(Some(v))
    }

    let get_lesser_func =
      get_lesser as fn(Option<Self::Item>, Self::Item) -> Option<Self::Item>;

    self
      .scan_initial(None, get_lesser_func)
      .last()
      // we can safely unwrap, because we will ever get this item
      // once a max value exists and is there.
      .map(|v| v.unwrap())
  }

  /// Calculates the sum of numbers emitted by an source observable and emits
  /// this sum when source completes.
  ///
  /// Emits zero when source completed as an and empty sequence.
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .sum()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // p rint log:
  /// // 5
  /// ```
  #[inline]
  fn sum(self) -> SumOp<Self, Self::Item>
  where
    Self::Item: Clone + Default + Add<Self::Item, Output = Self::Item>,
  {
    self.reduce(|acc, v| acc + v)
  }

  /// Emits the number of items emitted by a source observable when this source
  /// completes.
  ///
  /// The output type of this operator is fixed to [`usize`].
  ///
  /// Emits zero when source completed as an and empty sequence.
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec!['1', '7', '3', '0', '4'])
  ///   .count()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  #[inline]
  fn count(self) -> CountOp<Self, Self::Item> { self.reduce(|acc, _v| acc + 1) }

  /// Calculates the sum of numbers emitted by an source observable and emits
  /// this sum when source completes.
  ///
  /// Emits zero when source completed as an and empty sequence.
  /// Emits error when source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(vec![3., 4., 5., 6., 7.])
  ///   .average()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  #[inline]
  fn average(self) -> AverageOp<Self, Self::Item>
  where
    Self::Item: Clone
      + Send
      + Default
      + Add<Self::Item, Output = Self::Item>
      + Mul<f64, Output = Self::Item>,
  {
    /// Computing an average by multiplying accumulated nominator by a
    /// reciprocal of accumulated denominator. In this way some generic
    /// types that support linear scaling over floats values could be
    /// averaged (e.g. vectors)
    fn average_floats<T>(acc: Accum<T>) -> T
    where
      T: Default + Clone + Send + Mul<f64, Output = T>,
    {
      // Note: we will never be dividing by zero here, as
      // the acc.1 will be always >= 1.
      // It would have be zero if we've would have received an element
      // when the source observable is empty but beacuse of how
      // `scan` works, we will transparently not receive anything in
      // such case.
      acc.0 * (1.0 / (acc.1 as f64))
    }

    fn accumulate_item<T>(acc: Accum<T>, v: T) -> Accum<T>
    where
      T: Clone + Add<T, Output = T>,
    {
      let newacc = acc.0 + v;
      let newcount = acc.1 + 1;
      (newacc, newcount)
    }

    // our starting point
    let start = (Self::Item::default(), 0);

    let acc =
      accumulate_item as fn(Accum<Self::Item>, Self::Item) -> Accum<Self::Item>;
    let avg = average_floats as fn(Accum<Self::Item>) -> Self::Item;

    self.scan_initial(start, acc).last().map(avg)
  }

  /// Returns a ConnectableObservable. A ConnectableObservable Observable
  /// resembles an ordinary Observable, except that it does not begin emitting
  /// items when it is subscribed to, but only when the Connect operator is
  /// applied to it. In this way you can wait for all intended observers to
  /// subscribe to the Observable before the Observable begins emitting items.
  #[inline]
  fn publish<Subject: Default>(self) -> ConnectableObservable<Self, Subject> {
    ConnectableObservable {
      source: self,
      subject: Subject::default(),
    }
  }

  /// Returns a new Observable that multicast (shares) the original
  /// Observable. As long as there is at least one Subscriber this
  /// Observable will be subscribed and emitting data. When all subscribers
  /// have unsubscribed it will unsubscribe from the source Observable.
  /// Because the Observable is multicasting it makes the stream `hot`.
  /// This is an alias for `publish().ref_count()`
  #[inline]
  fn share<Subject, Inner>(
    self,
  ) -> RefCount<Inner, ConnectableObservable<Self, Subject>>
  where
    Inner: RefCountCreator<Connectable = ConnectableObservable<Self, Subject>>,
    Subject: Default,
    Self: Clone,
  {
    self.publish::<Subject>().ref_count::<Inner>()
  }

  /// Delays the emission of items from the source Observable by a given timeout
  /// or until a given `Instant`.
  #[inline]
  fn delay<SD>(self, dur: Duration, scheduler: SD) -> DelayOp<Self, SD> {
    DelayOp {
      source: self,
      delay: dur,
      scheduler,
    }
  }

  #[inline]
  fn delay_at<SD>(self, at: Instant, scheduler: SD) -> DelayOp<Self, SD> {
    DelayOp {
      source: self,
      delay: at.elapsed(),
      scheduler,
    }
  }

  /// Specify the Scheduler on which an Observable will operate
  ///
  /// With `SubscribeON` you can decide what type of scheduler a specific
  /// Observable will be using when it is subscribed to.
  ///
  /// Schedulers control the speed and order of emissions to observers from an
  /// Observable stream.
  ///
  /// # Example
  /// Given the following code:
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let a = observable::from_iter(1..5);
  /// let b = observable::from_iter(5..10);
  /// a.merge(b).subscribe(|v| print!("{} ", v));
  /// ```
  ///
  /// Both Observable `a` and `b` will emit their values directly and
  /// synchronously once they are subscribed to.
  /// This will result in the output of `1 2 3 4 5 6 7 8 9`.
  ///
  /// But if we instead use the `subscribe_on` operator declaring that we want
  /// to use the new thread scheduler for values emitted by Observable `a`:
  /// ```rust
  /// use rxrust::prelude::*;
  /// use std::thread;
  /// use futures::executor::ThreadPool;
  ///
  /// let pool = ThreadPool::new().unwrap();
  /// let a = observable::from_iter(1..5).subscribe_on(pool);
  /// let b = observable::from_iter(5..10);
  /// a.merge(b).into_shared().subscribe(|v|{
  ///   let handle = thread::current();
  ///   print!("{}({:?}) ", v, handle.id())
  /// });
  /// ```
  ///
  /// The output will instead by `1(thread 1) 2(thread 1) 3(thread 1) 4(thread
  /// 1)  5(thread 2) 6(thread 2) 7(thread 2) 8(thread 2) 9(thread id2)`.
  /// The reason for this is that Observable `b` emits its values directly like
  /// before, but the emissions from `a` are scheduled on a new thread because
  /// we are now using the `NewThread` Scheduler for that specific Observable.
  #[inline]
  fn subscribe_on<SD>(self, scheduler: SD) -> SubscribeOnOP<Self, SD> {
    SubscribeOnOP {
      source: self,
      scheduler,
    }
  }

  /// Re-emits all notifications from source Observable with specified
  /// scheduler.
  ///
  /// `ObserveOn` is an operator that accepts a scheduler as the parameter,
  /// which will be used to reschedule notifications emitted by the source
  /// Observable.
  #[inline]
  fn observe_on<SD>(self, scheduler: SD) -> ObserveOnOp<Self, SD> {
    ObserveOnOp {
      source: self,
      scheduler,
    }
  }

  /// Emits a value from the source Observable only after a particular time span
  /// has passed without another source emission.
  #[inline]
  fn debounce<SD>(
    self,
    duration: Duration,
    scheduler: SD,
  ) -> DebounceOp<Self, SD> {
    DebounceOp {
      source: self,
      duration,
      scheduler,
    }
  }

  /// Emits a value from the source Observable, then ignores subsequent source
  /// values for duration milliseconds, then repeats this process.
  ///
  /// #Example
  /// ```
  /// use rxrust::{ prelude::*, ops::throttle_time::ThrottleEdge };
  /// use std::time::Duration;
  /// use futures::executor::LocalPool;
  ///
  /// let mut local_scheduler = LocalPool::new();
  /// let spawner = local_scheduler.spawner();
  /// observable::interval(Duration::from_millis(1), spawner.clone())
  ///   .throttle_time(Duration::from_millis(9), ThrottleEdge::Leading, spawner)
  ///   .take(5)
  ///   .subscribe(move |v| println!("{}", v));
  ///
  /// local_scheduler.run();
  /// ```
  #[inline]
  fn throttle_time<SD>(
    self,
    duration: Duration,
    edge: ThrottleEdge,
    scheduler: SD,
  ) -> ThrottleTimeOp<Self, SD> {
    ThrottleTimeOp {
      source: self,
      duration,
      edge,
      scheduler,
    }
  }

  /// Returns an Observable that emits all items emitted by the source
  /// Observable that are distinct by comparison from previous items.
  #[inline]
  fn distinct(self) -> DistinctOp<Self> { DistinctOp { source: self } }

  /// 'Zips up' two observable into a single observable of pairs.
  ///
  /// zip() returns a new observable that will emit over two other
  /// observables,  returning a tuple where the first element comes from the
  /// first observable, and  the second element comes from the second
  /// observable.
  ///
  ///  In other words, it zips two observables together, into a single one.
  #[inline]
  fn zip<U>(self, other: U) -> ZipOp<Self, U>
  where
    U: Observable,
  {
    ZipOp { a: self, b: other }
  }

  /// Emits default value if Observable completed with empty result
  ///
  /// #Example
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::empty()
  ///   .default_if_empty(5)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // Prints:
  /// // 5
  /// ```
  #[inline]
  fn default_if_empty(
    self,
    default_value: Self::Item,
  ) -> DefaultIfEmptyOp<Self> {
    DefaultIfEmptyOp {
      source: self,
      is_empty: true,
      default_value,
    }
  }
}

pub trait LocalObservable<'a>: Observable {
  type Unsub: SubscriptionLike + 'static;
  fn actual_subscribe<O: Observer<Item = Self::Item, Err = Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub;
}

#[macro_export]
macro_rules! observable_proxy_impl {
    ($ty: ident, $host: ident$(, $lf: lifetime)?$(, $generics: ident) *) => {
  impl<$($lf, )? $host, $($generics ,)*> Observable
    for $ty<$($lf, )? $host, $($generics ,)*>
  where
    $host: Observable
  {
    type Item = $host::Item;
    type Err = $host::Err;
  }
}
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn smoke_element_at() {
    let s = observable::from_iter(0..20);
    s.clone().element_at(0).subscribe(|v| assert_eq!(v, 0));
    s.clone().element_at(5).subscribe(|v| assert_eq!(v, 5));
    s.clone().element_at(20).subscribe(|v| assert_eq!(v, 20));
    s.element_at(21).subscribe(|_| panic!());
  }

  #[test]
  fn bench_element_at() { do_bench_element_at(); }

  benchmark_group!(do_bench_element_at, element_at_bench);

  fn element_at_bench(b: &mut bencher::Bencher) { b.iter(smoke_element_at); }

  #[test]
  fn first() {
    let mut completed = 0;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first()
      .subscribe_complete(|_| next_count += 1, || completed += 1);

    assert_eq!(completed, 1);
    assert_eq!(next_count, 1);
  }

  #[test]
  fn bench_first() { do_bench_first(); }

  benchmark_group!(do_bench_first, first_bench);

  fn first_bench(b: &mut bencher::Bencher) { b.iter(first); }

  #[test]
  fn first_or() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first_or(100)
      .subscribe_complete(|_| next_count += 1, || completed = true);

    assert_eq!(next_count, 1);
    assert!(completed);

    completed = false;
    let mut v = 0;
    observable::empty()
      .first_or(100)
      .subscribe_complete(|value| v = value, || completed = true);

    assert!(completed);
    assert_eq!(v, 100);
  }

  #[test]
  fn bench_first_or() { do_bench_first_or(); }

  benchmark_group!(do_bench_first_or, first_or_bench);

  fn first_or_bench(b: &mut bencher::Bencher) { b.iter(first_or); }

  #[test]
  fn first_support_fork() {
    let mut value = 0;
    let mut value2 = 0;
    {
      let o = observable::from_iter(1..100).first();
      let o1 = o.clone().first();
      let o2 = o.first();
      o1.subscribe(|v| value = v);
      o2.subscribe(|v| value2 = v);
    }
    assert_eq!(value, 1);
    assert_eq!(value2, 1);
  }

  #[test]
  fn first_or_support_fork() {
    let mut default = 0;
    let mut default2 = 0;
    let o = observable::create(|mut subscriber| {
      subscriber.complete();
    })
    .first_or(100);
    let o1 = o.clone().first_or(0);
    let o2 = o.clone().first_or(0);
    o1.subscribe(|v| default = v);
    o2.subscribe(|v| default2 = v);
    assert_eq!(default, 100);
    assert_eq!(default, 100);
  }

  #[test]
  fn smoke_ignore_elements() {
    observable::from_iter(0..20)
      .ignore_elements()
      .subscribe(move |_| panic!());
  }

  #[test]
  fn bench_ignore() { do_bench_ignore(); }

  benchmark_group!(do_bench_ignore, ignore_emements_bench);

  fn ignore_emements_bench(b: &mut bencher::Bencher) {
    b.iter(smoke_ignore_elements);
  }

  #[test]
  fn shared_ignore_elements() {
    observable::from_iter(0..20)
      .ignore_elements()
      .into_shared()
      .subscribe(|_| panic!());
  }

  #[test]
  fn smoke_all() {
    observable::from_iter(0..10)
      .all(|v| v < 10)
      .subscribe(|b| assert!(b));
    observable::from_iter(0..10)
      .all(|v| v < 5)
      .subscribe(|b| assert!(!b));
  }

  #[test]
  fn bench_all() { do_bench_all(); }

  benchmark_group!(do_bench_all, all_bench);

  fn all_bench(b: &mut bencher::Bencher) { b.iter(smoke_all); }
}
