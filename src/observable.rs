#![macro_use]

mod trivial;
use std::hash::*;
use std::sync::Arc;
pub use trivial::*;
mod from_iter;
pub use from_iter::{from_iter, repeat};

pub mod of;
pub use of::{of, of_fn, of_option, of_result};

pub(crate) mod from_future;
pub use from_future::{from_future, from_future_result};

pub(crate) mod from_stream;
pub(crate) mod from_stream_result;
pub use from_stream::from_stream;
pub use from_stream_result::from_stream_result;

pub mod interval;
pub use interval::{interval, interval_at};

pub(crate) mod connectable_observable;
pub use connectable_observable::ConnectableObservable;

pub mod from_fn;
pub use from_fn::*;

pub mod timer;
pub use timer::{timer, timer_at};

pub mod start;
pub use start::start;

use crate::prelude::*;

mod subscribe_item;
pub use subscribe_item::*;
mod defer;
pub use defer::*;

use crate::ops::collect::CollectOp;
use crate::ops::combine_latest::CombineLatestOpThread;
use crate::ops::complete_status::{CompleteStatus, StatusOp};
use crate::ops::delay::{DelayOpThreads, DelaySubscriptionOp};
use crate::ops::finalize::FinalizeOpThreads;
use crate::ops::future::{ObservableFuture, ObservableFutureObserver};
use crate::ops::merge::MergeOpThreads;
use crate::ops::merge_all::MergeAllOpThreads;
use crate::ops::observe_on::ObserveOnOpThreads;
use crate::ops::on_complete::OnCompleteOp;
use crate::ops::on_error::OnErrorOp;
use crate::ops::ref_count::{ShareOp, ShareOpThreads};
use crate::ops::sample::SampleOpThreads;
use crate::ops::skip_until::SkipUntilOpThreads;
use crate::ops::stream::{ObservableStream, ObservableStreamObserver};
use crate::ops::take_until::TakeUntilOpThreads;
use crate::ops::timestamp::TimestampOp;
use crate::ops::with_latest_from::WithLatestFromOpThreads;
use crate::ops::zip::ZipOpThreads;
use crate::ops::FlatMapOpThreads;
pub use ops::box_it::BoxIt;

use crate::ops::default_if_empty::DefaultIfEmptyOp;
use crate::ops::distinct::{DistinctKeyOp, DistinctUntilKeyChangedOp};
use crate::ops::on_error_map::OnErrorMapOp;
use crate::ops::pairwise::PairwiseOp;
use crate::ops::tap::TapOp;
use ops::{
  buffer::{
    BufferOp, BufferWithCountOp, BufferWithCountOrTimerOp, BufferWithTimeOp,
  },
  combine_latest::CombineLatestOp,
  contains::ContainsOp,
  debounce::DebounceOp,
  delay::DelayOp,
  distinct::DistinctOp,
  distinct::DistinctUntilChangedOp,
  filter::FilterOp,
  filter_map::FilterMapOp,
  finalize::FinalizeOp,
  group_by::GroupByOp,
  last::LastOp,
  map::MapOp,
  map_to::MapToOp,
  merge::MergeOp,
  merge_all::MergeAllOp,
  observe_on::ObserveOnOp,
  sample::SampleOp,
  scan::ScanOp,
  skip::SkipOp,
  skip_last::SkipLastOp,
  skip_until::SkipUntilOp,
  skip_while::SkipWhileOp,
  start_with::StartWithOp,
  subscribe_on::SubscribeOnOP,
  take::TakeOp,
  take_last::TakeLastOp,
  take_until::TakeUntilOp,
  take_while::TakeWhileOp,
  throttle::{ThrottleEdge, ThrottleOp},
  with_latest_from::WithLatestFromOp,
  zip::ZipOp,
  Accum, AverageOp, CountOp, FlatMapOp, MinMaxOp, ReduceOp, SumOp,
};
use std::ops::{Add, Mul};
#[cfg(test)]
pub mod fake_timer;

type ALLOp<S, F, Item> = DefaultIfEmptyOp<
  TakeOp<FilterOp<MapOp<S, F, Item>, fn(&bool) -> bool>>,
  bool,
>;

pub trait Observable<Item, Err, O>
where
  O: Observer<Item, Err>,
{
  type Unsub: Subscription;

  fn actual_subscribe(self, observer: O) -> Self::Unsub;
}

pub trait ObservableExt<Item, Err>: Sized {
  /// emit only the first item emitted by an Observable
  #[inline]
  fn first(self) -> TakeOp<Self> {
    self.take(1)
  }

  /// emit only the first item emitted by an Observable
  fn first_or(self, default: Item) -> DefaultIfEmptyOp<TakeOp<Self>, Item> {
    DefaultIfEmptyOp::new(self.first(), default)
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
    default: Item,
  ) -> DefaultIfEmptyOp<LastOp<Self, Item>, Item> {
    DefaultIfEmptyOp::new(self.last(), default)
  }

  /// Emit only item n (0-indexed) emitted by an Observable
  #[inline]
  fn element_at(self, nth: usize) -> TakeOp<SkipOp<Self>> {
    TakeOp::new(self.skip(nth), 1)
  }

  /// Do not emit any items from an Observable but mirror its termination
  /// notification
  #[inline]
  fn ignore_elements(self) -> FilterOp<Self, fn(&Item) -> bool> {
    fn always_false<Item>(_: &Item) -> bool {
      false
    }
    self.filter(always_false as fn(&Item) -> bool)
  }

  /// Determine whether all items emitted by an Observable meet some criteria
  #[inline]
  fn all<F>(self, pred: F) -> ALLOp<Self, F, Item>
  where
    F: Fn(Item) -> bool,
  {
    fn not(b: &bool) -> bool {
      !b
    }
    let map: MapOp<Self, F, Item> = MapOp::new(self, pred);
    let filter_map = FilterOp {
      source: map,
      filter: not as fn(&bool) -> bool,
    };
    let take = TakeOp::new(filter_map, 1);
    DefaultIfEmptyOp::new(take, true)
  }

  /// Determine whether an Observable emits a particular item or not
  fn contains(self, target: Item) -> ContainsOp<Self, Item> {
    ContainsOp { source: self, target }
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
  fn last(self) -> LastOp<Self, Item> {
    LastOp { source: self, last: None }
  }

  /// Call a function when observable completes, errors or is unsubscribed from.
  #[inline]
  fn finalize<F>(self, f: F) -> FinalizeOp<Self, F>
  where
    F: FnMut(),
  {
    FinalizeOp::new(self, f)
  }

  /// A threads safe version of `finalize`
  fn finalize_threads<F>(self, f: F) -> FinalizeOpThreads<Self, F>
  where
    F: FnMut(),
  {
    FinalizeOpThreads::new(self, f)
  }

  /// Creates an Observable that combines all the emissions from Observables
  /// that get emitted from an Observable.
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// let mut source = Subject::default();
  /// let numbers = Subject::default();
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
  fn flatten<'a, Item2, Err2>(self) -> MergeAllOp<'a, Self, Item>
  where
    Item: ObservableExt<Item2, Err2>,
  {
    MergeAllOp::new(self, usize::MAX)
  }

  /// A threads safe version of `flatten`
  #[inline]
  fn flatten_threads<Item2, Err2>(self) -> MergeAllOpThreads<Self, Item>
  where
    Item: ObservableExt<Item2, Err2>,
  {
    MergeAllOpThreads::new(self, usize::MAX)
  }

  ///  Applies given function to each item emitted by this Observable, where
  ///  that function returns an Observable that itself emits items. It then
  ///  merges the emissions of these resulting Observables, emitting these
  ///  merged results as its own sequence.
  #[inline]
  fn flat_map<'a, V, Item2, F>(self, f: F) -> FlatMapOp<'a, Self, V, F, Item>
  where
    F: FnMut(Item) -> V,
    MapOp<Self, F, Item>: ObservableExt<V, Err>,
    V: ObservableExt<Item2, Err>,
  {
    self.map(f).merge_all(usize::MAX)
  }

  #[inline]
  fn concat_map<'a, V, Item2, F>(self, f: F) -> FlatMapOp<'a, Self, V, F, Item>
  where
    F: FnMut(Item) -> V,
    MapOp<Self, F, Item>: ObservableExt<V, Err>,
    V: ObservableExt<Item2, Err>,
  {
    self.map(f).concat_all()
  }

  #[inline]
  fn flat_map_threads<V, Item2, F>(
    self,
    f: F,
  ) -> FlatMapOpThreads<Self, V, F, Item>
  where
    F: FnMut(Item) -> V,
    MapOp<Self, F, Item>: ObservableExt<V, Err>,
    V: ObservableExt<Item2, Err>,
  {
    self.map(f).merge_all_threads(usize::MAX)
  }

  #[inline]
  fn concat_map_threads<V, Item2, F>(
    self,
    f: F,
  ) -> FlatMapOpThreads<Self, V, F, Item>
  where
    F: FnMut(Item) -> V,
    MapOp<Self, F, Item>: ObservableExt<V, Err>,
    V: ObservableExt<Item2, Err>,
  {
    self.map(f).concat_all_threads()
  }

  /// Groups items emitted by the source Observable into Observables.
  /// Each emitted Observable emits items matching the key returned
  /// by the discriminator function.
  ///
  /// # Example
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// #[derive(Clone)]
  /// struct Person {
  ///   name: String,
  ///   age: u32,
  /// }
  ///
  /// observable::from_iter([
  ///   Person{ name: String::from("John"), age: 26 },
  ///   Person{ name: String::from("Anne"), age: 28 },
  ///   Person{ name: String::from("Gregory"), age: 24 },
  ///   Person{ name: String::from("Alice"), age: 28 },
  /// ])
  /// .group_by::<_,_,Subject<_,_>>(|person: &Person| person.age)
  /// .flat_map(|group| group.reduce(|acc, person: Person| format!("{} {}", acc, person.name)))
  /// .subscribe(|result| println!("{}", result));
  ///
  /// // Prints:
  /// //  John
  /// //  Anne Alice
  /// //  Gregory
  /// ```
  #[inline]
  fn group_by<D, Key, Subject>(self, discr: D) -> GroupByOp<Self, D, Subject>
  where
    D: FnMut(&Item) -> Key,
    Key: Hash + Eq + Clone,
    Subject: Clone + Default + Observer<Item, Err>,
  {
    GroupByOp::new(self, discr)
  }

  /// Creates a new stream which calls a closure on each element and uses
  /// its return as the value.
  #[inline]
  fn map<B, F>(self, f: F) -> MapOp<Self, F, Item>
  where
    F: FnMut(Item) -> B,
  {
    MapOp::new(self, f)
  }

  /// Creates a new stream which calls a closure on each error and uses
  /// its return as emitted error.
  #[inline]
  fn on_error_map<B, F>(self, f: F) -> OnErrorMapOp<Self, F, Err>
  where
    F: FnMut(Err) -> B,
  {
    OnErrorMapOp::new(self, f)
  }

  /// Maps emissions to a constant value.
  #[inline]
  fn map_to<B>(self, value: B) -> MapToOp<Self, B, Item> {
    MapToOp::new(self, value)
  }

  /// Creates a new stream which maps each element to a tuple of the element
  /// and the time that it was observed.
  #[inline]
  fn timestamp(self) -> TimestampOp<Self, Item> {
    fn timestamp<Item>(v: Item) -> (Item, Instant) {
      (v, Instant::now())
    }
    self.map(timestamp)
  }

  /// combine two Observables into one by merging their emissions
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// let numbers = Subject::default();
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
  fn merge<S>(self, other: S) -> MergeOp<Self, S>
  where
    S: ObservableExt<Item, Err>,
  {
    MergeOp::new(self, other)
  }

  /// A threads safe version of `merge`
  #[inline]
  fn merge_threads<S>(self, other: S) -> MergeOpThreads<Self, S>
  where
    S: ObservableExt<Item, Err>,
  {
    MergeOpThreads::new(self, other)
  }

  /// Converts a higher-order Observable into a first-order Observable which
  /// concurrently delivers all values that are emitted on the inner
  /// Observables.
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// # use futures::executor::LocalPool;
  /// # use std::time::Duration;
  /// let mut local = LocalPool::new();
  /// observable::from_iter(
  ///   (0..3)
  ///     .map(|_| interval(Duration::from_millis(1), local.spawner()).take(5)),
  /// )
  /// .merge_all(2)
  /// .subscribe(move |i| println!("{}", i));
  /// local.run();
  /// ```
  #[inline]
  fn merge_all<'a, Item2>(self, concurrent: usize) -> MergeAllOp<'a, Self, Item>
  where
    Item: ObservableExt<Item2, Err>,
  {
    MergeAllOp::new(self, concurrent)
  }

  #[inline]
  fn concat_all<'a, Item2>(self) -> MergeAllOp<'a, Self, Item>
  where
    Item: ObservableExt<Item2, Err>,
  {
    MergeAllOp::new(self, 1)
  }

  /// A threads safe version of `merge_all`
  #[inline]
  fn merge_all_threads<Item2>(
    self,
    concurrent: usize,
  ) -> MergeAllOpThreads<Self, Item>
  where
    Item: ObservableExt<Item2, Err>,
  {
    MergeAllOpThreads::new(self, concurrent)
  }

  #[inline]
  fn concat_all_threads<Item2>(self) -> MergeAllOpThreads<Self, Item>
  where
    Item: ObservableExt<Item2, Err>,
  {
    MergeAllOpThreads::new(self, 1)
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
    F: Fn(&Item) -> bool,
  {
    FilterOp { source: self, filter }
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
  fn filter_map<F, OutputItem>(self, f: F) -> FilterMapOp<Self, F, Item>
  where
    F: FnMut(Item) -> Option<OutputItem>,
  {
    FilterMapOp::new(self, f)
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
  fn skip(self, count: usize) -> SkipOp<Self> {
    SkipOp::new(self, count)
  }

  /// Discard items emitted by an Observable until a second Observable emits an item
  ///
  /// # Example
  /// Ignore the numbers in the 0-10 range until the Observer emits 5 and trigger
  ///  the notify observable.
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// let mut items = vec![];
  /// let notifier = Subject::<(), ()>::default();
  /// let mut c_notifier = notifier.clone();
  /// observable::from_iter(0..10)
  ///   .tap(move |v| {
  ///     if v == &5 {
  ///       c_notifier.next(());
  ///     }
  ///   })
  ///   .skip_until(notifier)
  ///   .subscribe(|v| items.push(v));
  ///
  /// assert_eq!((5..10).collect::<Vec<i32>>(), items);
  /// ```
  #[inline]
  fn skip_until<NotifyItem, NotifyErr, Other>(
    self,
    notifier: Other,
  ) -> SkipUntilOp<Self, Other, NotifyItem, NotifyErr>
  where
    Other: ObservableExt<NotifyItem, NotifyErr>,
  {
    SkipUntilOp::new(self, notifier)
  }

  /// A threads safe version of `skip_until_threads`
  #[inline]
  fn skip_until_threads<NotifyItem, NotifyErr, Other>(
    self,
    notifier: Other,
  ) -> SkipUntilOpThreads<Self, Other, NotifyItem, NotifyErr>
  where
    Other: ObservableExt<NotifyItem, NotifyErr>,
  {
    SkipUntilOpThreads::new(self, notifier)
  }

  /// Discard items emitted by an Observable until a specified condition becomes false
  ///
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
  fn skip_while<F>(self, predicate: F) -> SkipWhileOp<Self, F>
  where
    F: FnMut(&Item) -> bool,
  {
    SkipWhileOp { source: self, predicate }
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
    SkipLastOp { source: self, count }
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
  fn take(self, count: usize) -> TakeOp<Self> {
    TakeOp::new(self, count)
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
  fn take_until<Notify, NotifyItem, NotifyErr>(
    self,
    notifier: Notify,
  ) -> TakeUntilOp<Self, Notify, NotifyItem, NotifyErr> {
    TakeUntilOp::new(self, notifier)
  }

  #[inline]
  fn take_until_threads<Notify, NotifyItem, NotifyErr>(
    self,
    notifier: Notify,
  ) -> TakeUntilOpThreads<Self, Notify, NotifyItem, NotifyErr> {
    TakeUntilOpThreads::new(self, notifier)
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
    F: FnMut(&Item) -> bool,
  {
    TakeWhileOp { source: self, callback, inclusive: false }
  }

  /// Emits values while result of an callback is true and the last one that
  /// causes the callback to return false.
  ///
  /// # Example
  /// Take the first 5 seconds of an infinite 1-second interval Observable
  ///
  /// ```
  /// # use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..10)
  ///   .take_while_inclusive(|v| v < &4)
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
  fn take_while_inclusive<F>(self, callback: F) -> TakeWhileOp<Self, F>
  where
    F: FnMut(&Item) -> bool,
  {
    TakeWhileOp { source: self, callback, inclusive: true }
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
    TakeLastOp { source: self, count }
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
  fn sample<Sample, SampleItem, SampleErr>(
    self,
    sampling: Sample,
  ) -> SampleOp<Self, Sample, SampleItem>
  where
    Sample: ObservableExt<SampleItem, SampleErr>,
  {
    SampleOp::new(self, sampling)
  }

  /// A threads safe version of `sample`
  #[inline]
  fn sample_threads<Sample, SampleItem, SampleErr>(
    self,
    sampling: Sample,
  ) -> SampleOpThreads<Self, Sample, SampleItem>
  where
    Sample: ObservableExt<SampleItem, SampleErr>,
  {
    SampleOpThreads::new(self, sampling)
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
  ) -> ScanOp<Self, BinaryOp, OutputItem, Item>
  where
    BinaryOp: Fn(OutputItem, Item) -> OutputItem,
    OutputItem: Clone,
  {
    ScanOp::new(self, binary_op, initial_value)
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
  ) -> ScanOp<Self, BinaryOp, OutputItem, Item>
  where
    BinaryOp: Fn(OutputItem, Item) -> OutputItem,
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
  ) -> ReduceOp<Self, BinaryOp, OutputItem, Item>
  where
    BinaryOp: Fn(OutputItem, Item) -> OutputItem,
    OutputItem: Clone,
  {
    // realised as a composition of `scan`, and `last`
    let scan = self.scan_initial(initial.clone(), binary_op);
    let last = LastOp { source: scan, last: None };
    DefaultIfEmptyOp::new(last, initial)
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
  ) -> ReduceOp<Self, BinaryOp, OutputItem, Item>
  where
    BinaryOp: Fn(OutputItem, Item) -> OutputItem,
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
  fn max(self) -> MinMaxOp<Self, Item>
  where
    Item: PartialOrd<Item> + Clone,
  {
    let max_fn = |max, v| match max {
      Some(max) if max > v => Some(max),
      _ => Some(v),
    };
    let scan =
      self.scan_initial(None, max_fn as fn(Option<Item>, Item) -> Option<Item>);
    let last = LastOp::new(scan);

    // we can safely unwrap, because we will ever get this item
    // once a max value exists and is there.
    MapOp::new(last, |v| v.unwrap())
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
  fn min(self) -> MinMaxOp<Self, Item>
  where
    Item: Clone + PartialOrd<Item>,
  {
    let min_fn = |min, v| match min {
      Some(min) if min < v => Some(min),
      _ => Some(v),
    };
    let scan =
      self.scan_initial(None, min_fn as fn(Option<Item>, Item) -> Option<Item>);
    let last = LastOp::new(scan);

    // we can safely unwrap, because we will ever get this item
    // once a max value exists and is there.
    MapOp::new(last, |v| v.unwrap())
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
  fn sum(self) -> SumOp<Self, Item>
  where
    Item: Clone + Default + Add<Item, Output = Item>,
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
  fn count(self) -> CountOp<Self, Item> {
    self.reduce(|acc, _v| acc + 1)
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
  /// observable::from_iter(vec![3., 4., 5., 6., 7.])
  ///   .average()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 5
  /// ```
  #[inline]
  fn average(self) -> AverageOp<Self, Item>
  where
    Item: Clone + Default + Add<Item, Output = Item> + Mul<f64, Output = Item>,
    ScanOp<Self, fn(Accum<Item>, Item) -> Accum<Item>, Accum<Item>, Item>:
      ObservableExt<Accum<Item>, Err>,
    LastOp<
      ScanOp<Self, fn(Accum<Item>, Item) -> Accum<Item>, Accum<Item>, Item>,
      Accum<Item>,
    >: ObservableExt<Accum<Item>, Err>,
  {
    /// Computing an average by multiplying accumulated nominator by a
    /// reciprocal of accumulated denominator. In this way some generic
    /// types that support linear scaling over floats values could be
    /// averaged (e.g. vectors)
    fn average_floats<T>(acc: Accum<T>) -> T
    where
      T: Default + Clone + Mul<f64, Output = T>,
    {
      // Note: we will never be dividing by zero here, as
      // the acc.1 will be always >= 1.
      // It would have be zero if we've would have received an element
      // when the source observable is empty but because of how
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
    let start = (Item::default(), 0);
    let acc = accumulate_item as fn(Accum<Item>, Item) -> Accum<Item>;
    let avg = average_floats as fn(Accum<Item>) -> Item;
    self.scan_initial(start, acc).last().map(avg)
  }

  /// Returns a ConnectableObservable. A ConnectableObservable Observable
  /// resembles an ordinary Observable, except that it does not begin emitting
  /// items when it is subscribed to, but only when the Connect operator is
  /// applied to it. In this way you can wait for all intended observers to
  /// subscribe to the Observable before the Observable begins emitting items.
  #[inline]
  fn publish<Subject: Default>(self) -> ConnectableObservable<Self, Subject> {
    ConnectableObservable::new(self)
  }

  /// Returns a new Observable that multicast (shares) the original
  /// Observable. As long as there is at least one Subscriber this
  /// Observable will be subscribed and emitting data. When all subscribers
  /// have unsubscribed it will unsubscribe from the source Observable.
  /// Because the Observable is multicasting it makes the stream `hot`.
  /// This is an alias for `publish().ref_count()`
  #[inline]
  fn share<'a>(self) -> ShareOp<'a, Item, Err, Self> {
    ShareOp::new(self)
  }

  #[inline]
  fn share_threads(self) -> ShareOpThreads<Item, Err, Self> {
    ShareOpThreads::new(self)
  }

  /// Delays the emission of items from the source Observable by a given timeout
  /// or until a given `Instant`.
  #[inline]
  fn delay<SD>(self, dur: Duration, scheduler: SD) -> DelayOp<Self, SD> {
    DelayOp { source: self, delay: dur, scheduler }
  }

  /// A threads safe version of `delay`
  #[inline]
  fn delay_threads<SD>(
    self,
    dur: Duration,
    scheduler: SD,
  ) -> DelayOpThreads<Self, SD> {
    DelayOpThreads { source: self, delay: dur, scheduler }
  }

  #[inline]
  fn delay_at<SD>(self, at: Instant, scheduler: SD) -> DelayOp<Self, SD> {
    DelayOp {
      source: self,
      delay: at.elapsed(),
      scheduler,
    }
  }

  /// A threads safe version of `delay_at`
  #[inline]
  fn delay_at_threads<SD>(
    self,
    at: Instant,
    scheduler: SD,
  ) -> DelayOpThreads<Self, SD> {
    DelayOpThreads {
      source: self,
      delay: at.elapsed(),
      scheduler,
    }
  }

  /// It's similar to delay but rather than timeshifting the emissions from
  /// the source Observable, it timeshifts the moment of subscription to that
  /// Observable.
  #[inline]
  fn delay_subscription<SD>(
    self,
    dur: Duration,
    scheduler: SD,
  ) -> DelaySubscriptionOp<Self, SD> {
    DelaySubscriptionOp { source: self, delay: dur, scheduler }
  }

  #[inline]
  fn delay_subscription_at<SD>(
    self,
    at: Instant,
    scheduler: SD,
  ) -> DelaySubscriptionOp<Self, SD> {
    DelaySubscriptionOp {
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
  ///
  /// let pool = FuturesThreadPoolScheduler::new().unwrap();
  /// let a = observable::from_iter(1..5).subscribe_on(pool);
  /// let b = observable::from_iter(5..10);
  /// a.merge_threads(b).subscribe(|v|{
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
    SubscribeOnOP { source: self, scheduler }
  }

  /// Re-emits all notifications from source Observable with specified
  /// scheduler.
  ///
  /// `ObserveOn` is an operator that accepts a scheduler as the parameter,
  /// which will be used to reschedule notifications emitted by the source
  /// Observable.
  #[inline]
  fn observe_on<SD>(self, scheduler: SD) -> ObserveOnOp<Self, SD> {
    ObserveOnOp { source: self, scheduler }
  }

  /// A thread safe version of `observe_on`
  #[inline]
  fn observe_on_threads<SD>(
    self,
    scheduler: SD,
  ) -> ObserveOnOpThreads<Self, SD> {
    ObserveOnOpThreads { source: self, scheduler }
  }

  /// Emits a value from the source Observable only after a particular time span
  /// has passed without another source emission.
  #[inline]
  fn debounce<SD>(
    self,
    duration: Duration,
    scheduler: SD,
  ) -> DebounceOp<Self, SD> {
    DebounceOp { source: self, duration, scheduler }
  }

  /// Emits a value from the source Observable, then ignores subsequent source
  /// values for duration milliseconds, then repeats this process.
  ///
  /// #Example
  /// ```
  /// use rxrust::{ prelude::*, ops::throttle::ThrottleEdge };
  /// use std::time::Duration;
  ///
  /// let mut local_pool = FuturesLocalSchedulerPool::new();
  /// let scheduler = local_pool.spawner();
  /// observable::interval(Duration::from_millis(1), scheduler.clone())
  ///   .throttle(
  ///     |val| -> Duration {
  ///       if val % 2 == 0 {
  ///         Duration::from_millis(7)
  ///       } else {
  ///         Duration::from_millis(5)
  ///       }
  ///     },
  ///     ThrottleEdge::leading(), scheduler)
  ///   .take(5)
  ///   .subscribe(move |v| println!("{}", v));
  ///
  /// local_pool.run();
  /// ```
  #[inline]
  fn throttle<SD, F>(
    self,
    duration_selector: F,
    edge: ThrottleEdge,
    scheduler: SD,
  ) -> ThrottleOp<Self, SD, F>
  where
    F: Fn(&Item) -> Duration,
  {
    ThrottleOp {
      source: self,
      duration_selector,
      edge,
      scheduler,
    }
  }

  /// Emits a value from the source Observable, then ignores subsequent source
  /// values for duration milliseconds, then repeats this process.
  ///
  /// #Example
  /// ```
  /// use rxrust::{ prelude::*, ops::throttle::ThrottleEdge };
  /// use std::time::Duration;
  ///
  /// let mut local_pool = FuturesLocalSchedulerPool::new();
  /// let scheduler = local_pool.spawner();
  /// observable::interval(Duration::from_millis(1), scheduler.clone())
  ///   .throttle_time(
  ///     Duration::from_millis(9), ThrottleEdge::leading(), scheduler)
  ///   .take(5)
  ///   .subscribe(move |v| println!("{}", v));
  ///
  /// // wait task finish.
  /// local_pool.run();
  /// ```
  #[inline]
  #[allow(clippy::type_complexity)]
  fn throttle_time<SD>(
    self,
    duration: Duration,
    edge: ThrottleEdge,
    scheduler: SD,
  ) -> ThrottleOp<Self, SD, Box<dyn Fn(&Item) -> Duration + Send + Sync>>
  where
    Item: 'static,
  {
    self.throttle(Box::new(move |_| duration), edge, scheduler)
  }

  /// Returns an Observable that emits all items emitted by the source
  /// Observable that are distinct by comparison from previous items.
  #[inline]
  fn distinct(self) -> DistinctOp<Self> {
    DistinctOp { source: self }
  }

  /// Variant of distinct that takes a key selector.
  #[inline]
  fn distinct_key<F>(self, key: F) -> DistinctKeyOp<Self, F> {
    DistinctKeyOp { source: self, key }
  }

  /// Only emit when the current value is different than the last
  #[inline]
  fn distinct_until_changed(self) -> DistinctUntilChangedOp<Self> {
    DistinctUntilChangedOp { source: self }
  }

  /// Variant of distinct_until_changed that takes a key selector.
  #[inline]
  fn distinct_until_key_changed<F>(
    self,
    key: F,
  ) -> DistinctUntilKeyChangedOp<Self, F> {
    DistinctUntilKeyChangedOp { source: self, key }
  }

  /// 'Zips up' two observable into a single observable of pairs.
  ///
  /// zip() returns a new observable that will emit over two other
  /// observables,  returning a tuple where the first element comes from the
  /// first observable, and  the second element comes from the second
  /// observable.
  ///
  ///  In other words, it zips two observables together, into a single one.
  #[inline]
  fn zip<Other, Item2>(self, other: Other) -> ZipOp<Self, Other>
  where
    Other: ObservableExt<Item2, Err>,
  {
    ZipOp::new(self, other)
  }

  /// A threads safe version of `zip`
  #[inline]
  fn zip_threads<Other, Item2>(self, other: Other) -> ZipOpThreads<Self, Other>
  where
    Other: ObservableExt<Item2, Err>,
  {
    ZipOpThreads::new(self, other)
  }

  /// Combines the source Observable with other Observables to create an
  /// Observable whose values are calculated from the latest values of each,
  /// only when the source emits.
  ///
  /// Whenever the source Observable emits a value, it computes a formula
  /// using that value plus the latest values from other input Observables,
  /// then emits the output of that formula.
  #[inline]
  fn with_latest_from<From, OtherItem>(
    self,
    from: From,
  ) -> WithLatestFromOp<Self, From>
  where
    From: ObservableExt<OtherItem, Err>,
    OtherItem: Clone,
  {
    WithLatestFromOp::new(self, from)
  }

  #[inline]
  fn with_latest_from_threads<From, OtherItem>(
    self,
    from: From,
  ) -> WithLatestFromOpThreads<Self, From>
  where
    From: ObservableExt<OtherItem, Err>,
    OtherItem: Clone,
  {
    WithLatestFromOpThreads::new(self, from)
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
    default_value: Item,
  ) -> DefaultIfEmptyOp<Self, Item> {
    DefaultIfEmptyOp::new(self, default_value)
  }

  #[inline]
  fn buffer<N>(self, closing_notifier: N) -> BufferOp<Self, N>
  where
    N: ObservableExt<(), Err>,
  {
    BufferOp { source: self, closing_notifier }
  }

  /// Buffers emitted values of type T in a Vec<T> and
  /// emits that Vec<T> as soon as the buffer's size equals
  /// the given count.
  /// On complete, if the buffer is not empty,
  /// it will be emitted.
  /// On error, the buffer will be discarded.
  ///
  /// The operator never returns an empty buffer.
  ///
  /// #Example
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// observable::from_iter(0..6)
  ///   .buffer_with_count(3)
  ///   .subscribe(|vec| println!("{:?}", vec));
  ///
  /// // Prints:
  /// // [0, 1, 2]
  /// // [3, 4, 5]
  /// ```
  #[inline]
  fn buffer_with_count(self, count: usize) -> BufferWithCountOp<Self> {
    BufferWithCountOp { source: self, count }
  }

  /// Buffers emitted values of type T in a Vec<T> and
  /// emits that Vec<T> periodically.
  ///
  /// On complete, if the buffer is not empty,
  /// it will be emitted.
  /// On error, the buffer will be discarded.
  ///
  /// The operator never returns an empty buffer.
  ///
  /// #Example
  /// ```
  /// use rxrust::prelude::*;
  /// use std::time::Duration;
  ///
  /// let pool = FuturesThreadPoolScheduler::new().unwrap();
  ///
  /// observable::create(|mut subscriber: SubscriberThreads<_>| {
  ///   subscriber.next(0);
  ///   subscriber.next(1);
  ///   std::thread::sleep(Duration::from_millis(100));
  ///   subscriber.next(2);
  ///   subscriber.next(3);
  ///   subscriber.complete();
  /// })
  ///   .buffer_with_time(Duration::from_millis(50), pool)
  ///   .subscribe(|vec| println!("{:?}", vec));
  ///
  /// // Prints:
  /// // [0, 1]
  /// // [2, 3]
  /// ```
  #[inline]
  fn buffer_with_time<S>(
    self,
    time: Duration,
    scheduler: S,
  ) -> BufferWithTimeOp<Self, S> {
    BufferWithTimeOp { source: self, time, scheduler }
  }

  /// Buffers emitted values of type T in a Vec<T> and
  /// emits that Vec<T> either if the buffer's size equals count, or
  /// periodically. This operator combines the functionality of
  /// buffer_with_count and buffer_with_time.
  ///
  /// #Example
  /// ```
  /// use rxrust::prelude::*;
  /// use std::time::Duration;
  ///
  /// let pool = FuturesThreadPoolScheduler::new().unwrap();
  ///
  /// observable::create(|mut subscriber: SubscriberThreads<_>| {
  ///   subscriber.next(0);
  ///   subscriber.next(1);
  ///   subscriber.next(2);
  ///   std::thread::sleep(Duration::from_millis(100));
  ///   subscriber.next(3);
  ///   subscriber.next(4);
  ///   subscriber.complete();
  /// })
  ///   .buffer_with_count_and_time(2, Duration::from_millis(50), pool)
  ///   .subscribe(|vec| println!("{:?}", vec));
  ///
  /// // Prints:
  /// // [0, 1]
  /// // [2]
  /// // [3, 4]
  /// ```
  #[inline]
  fn buffer_with_count_and_time<S>(
    self,
    count: usize,
    time: Duration,
    scheduler: S,
  ) -> BufferWithCountOrTimerOp<Self, S> {
    BufferWithCountOrTimerOp { source: self, count, time, scheduler }
  }

  /// Emits item which is combining latest items from two observables.
  ///
  /// combine_latest() merges two observables into one observable
  /// by applying a binary operator on the latest item of two observable
  /// whenever each of observables produces an element.
  ///
  /// #Example
  /// ```
  /// use rxrust::prelude::*;
  /// use std::time::Duration;
  /// use futures::executor::LocalPool;
  ///
  /// let mut local_scheduler = LocalPool::new();
  /// let spawner = local_scheduler.spawner();
  /// observable::interval(Duration::from_millis(2), spawner.clone())
  ///   .combine_latest(
  ///     observable::interval(Duration::from_millis(3), spawner),
  ///     |a, b| (a, b),
  ///   )
  ///   .take(5)
  ///   .subscribe(move |v| println!("{}, {}", v.0, v.1));
  ///
  /// local_scheduler.run();
  /// // print logs:
  /// // 0, 0
  /// // 1, 0
  /// // 2, 0
  /// // 2, 1
  /// // 3, 1
  /// ```
  fn combine_latest<Other, OtherItem, BinaryOp, OutputItem>(
    self,
    other: Other,
    binary_op: BinaryOp,
  ) -> CombineLatestOp<Self, Other, BinaryOp>
  where
    Other: ObservableExt<OtherItem, Err>,
    BinaryOp: FnMut(Item, OtherItem) -> OutputItem,
  {
    CombineLatestOp::new(self, other, binary_op)
  }

  fn combine_latest_threads<Other, OtherItem, BinaryOp, OutputItem>(
    self,
    other: Other,
    binary_op: BinaryOp,
  ) -> CombineLatestOpThread<Self, Other, BinaryOp>
  where
    Other: ObservableExt<OtherItem, Err>,
    BinaryOp: FnMut(Item, OtherItem) -> OutputItem,
  {
    CombineLatestOpThread::new(self, other, binary_op)
  }

  /// Returns an observable that, at the moment of subscription, will
  /// synchronously emit all values provided to this operator, then subscribe
  /// to the source and mirror all of its emissions to subscribers.
  fn start_with<B>(self, values: Vec<B>) -> StartWithOp<Self, B> {
    StartWithOp { source: self, values }
  }

  /// Groups pairs of consecutive emissions together and emits them as an pair
  /// of two values.
  fn pairwise(self) -> PairwiseOp<Self> {
    PairwiseOp { source: self }
  }

  /// Used to perform side-effects for notifications from the source observable
  #[inline]
  fn tap<F>(self, f: F) -> TapOp<Self, F>
  where
    F: FnMut(&Item),
  {
    TapOp { source: self, func: f }
  }

  /// Process the error of the observable and the return observable can't catch the error any more.
  #[inline]
  #[must_use]
  fn on_error<F>(self, f: F) -> OnErrorOp<Self, F, Err>
  where
    F: FnOnce(Err),
  {
    OnErrorOp::new(self, f)
  }

  #[inline]
  #[must_use]
  fn on_complete<F>(self, f: F) -> OnCompleteOp<Self, F>
  where
    F: FnOnce(),
  {
    OnCompleteOp { source: self, func: f }
  }

  /// Turn the observable to an new observable that will track its complete
  /// status.
  /// The second element of return tuple provide ability let you can query if it
  /// completed  or error occur. You can also wait the observable finish.
  #[inline]
  fn complete_status(self) -> (StatusOp<Self>, Arc<CompleteStatus>) {
    ops::complete_status::complete_status(self)
  }

  /// Collects all the items emitted by the observable into a collection.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut subject = Subject::default();
  /// subject.clone()
  ///   .collect::<Vec<_>>()
  ///   .subscribe(|values| {
  ///     println!("{values:?}");
  /// });
  ///
  /// subject.next(2);
  /// subject.next(4);
  /// subject.next(6);
  ///
  /// // prints: [2,4,6]
  /// ```
  #[inline]
  fn collect<C>(self) -> CollectOp<Self, C>
  where
    C: IntoIterator + Extend<C::Item> + Default,
  {
    self.collect_into(C::default())
  }

  /// Collects all the items emitted by the observable into the given collection.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// #[tokio::main]
  /// async fn main() {
  ///   let observable = observable::from_iter(['x', 'y', 'z']);
  ///   let base = vec!['a', 'b', 'c'];
  ///   let values = observable.collect_into::<Vec<_>>(base)
  ///       .to_future()
  ///       .await
  ///       .unwrap()
  ///       .ok();
  ///
  ///   assert_eq!(values, Some(vec!['a', 'b', 'c', 'x', 'y', 'z']));
  /// }
  /// ```
  #[inline]
  fn collect_into<C>(self, collection: C) -> CollectOp<Self, C>
  where
    C: IntoIterator + Extend<C::Item>,
  {
    CollectOp::new(self, collection)
  }

  /// Converts this observable into a `Future` that resolves to `Result<Result<Item, Err>, ObservableError>`.
  ///
  /// # Error
  /// - ObservableError::Empty: If the observable emitted no values.
  /// - Observable::MultipleValues: If the observable emitted more than one value.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// #[tokio::main]
  /// async fn main() {
  ///   let observable = observable::of(12);
  ///   let value = observable.to_future().await.unwrap().ok();
  ///   assert_eq!(value, Some(12));
  /// }
  /// ```
  #[inline]
  fn to_future(self) -> ObservableFuture<Item, Err>
  where
    Self: Observable<Item, Err, ObservableFutureObserver<Item, Err>>,
  {
    ObservableFuture::new(self)
  }

  /// Converts this observable into a stream that emits the values of the observable.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  /// use futures::StreamExt;
  ///
  /// #[tokio::main]
  /// async fn main() {
  ///   let observable = observable::from_iter([1,2,3]);
  ///   let mut stream = observable.to_stream();
  ///   let mut values = vec![];
  ///
  ///   while let Some(Ok(x)) = stream.next().await {
  ///     values.push(x);
  ///   }
  ///
  ///   assert_eq!(values, vec![1,2,3]);
  /// }
  /// ```
  #[inline]
  fn to_stream(self) -> ObservableStream<Item, Err>
  where
    Self: Observable<Item, Err, ObservableStreamObserver<Item, Err>>,
  {
    ObservableStream::new(self)
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
  fn bench_element_at() {
    do_bench_element_at();
  }

  benchmark_group!(do_bench_element_at, element_at_bench);

  fn element_at_bench(b: &mut bencher::Bencher) {
    b.iter(smoke_element_at);
  }

  #[test]
  fn first() {
    let mut completed = 0;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first()
      .on_complete(|| completed += 1)
      .subscribe(|_| next_count += 1);

    assert_eq!(completed, 1);
    assert_eq!(next_count, 1);
  }

  #[test]
  fn bench_first() {
    do_bench_first();
  }

  benchmark_group!(do_bench_first, first_bench);

  fn first_bench(b: &mut bencher::Bencher) {
    b.iter(first);
  }

  #[test]
  fn first_or() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..2)
      .first_or(100)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 1);
    assert!(completed);

    completed = false;
    let mut v = 0;
    observable::empty()
      .first_or(100)
      .on_complete(|| completed = true)
      .subscribe(|value| v = value);

    assert!(completed);
    assert_eq!(v, 100);
  }

  #[test]
  fn bench_first_or() {
    do_bench_first_or();
  }

  benchmark_group!(do_bench_first_or, first_or_bench);

  fn first_or_bench(b: &mut bencher::Bencher) {
    b.iter(first_or);
  }

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
    let o = observable::create(|subscriber: Subscriber<_>| {
      subscriber.complete();
    })
    .first_or(100);
    let o1 = o.clone().first_or(0);
    let o2 = o.clone().first_or(0);
    let u1: Box<dyn FnMut(i32) + '_> = Box::new(|v| default = v);
    let u2: Box<dyn FnMut(i32) + '_> = Box::new(|v| default2 = v);
    o1.subscribe(u1);
    o2.subscribe(u2);
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
  fn bench_ignore() {
    do_bench_ignore();
  }

  benchmark_group!(do_bench_ignore, ignore_emements_bench);

  fn ignore_emements_bench(b: &mut bencher::Bencher) {
    b.iter(smoke_ignore_elements);
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
  fn bench_all() {
    do_bench_all();
  }

  benchmark_group!(do_bench_all, all_bench);

  fn all_bench(b: &mut bencher::Bencher) {
    b.iter(smoke_all);
  }

  #[test]
  fn timestamp() {
    let mut vs = Vec::new();
    let mut ts = Vec::new();

    let before = Instant::now();

    observable::from_iter(0..3).timestamp().subscribe(|(v, t)| {
      vs.push(v);
      ts.push(t);
    });

    let after = Instant::now();

    assert_eq!(vs, vec![0, 1, 2]);
    assert_eq!(ts.len(), 3);

    for i in 0..3 {
      assert!(
        ts[i] <= after,
        "element {} in accumulated timestamps ({:#?}) was after end ({:#?})",
        i,
        ts,
        after
      );
      assert!(
        ts[i] >= before,
        "element {} in accumulated timestamps ({:#?}) was before start ({:#?})",
        i,
        ts,
        after
      );
    }

    for i in 1..3 {
      assert!(
        ts[i - 1] <= ts[i],
        "element {} ({:#?}) is not after previous element ({:#?})",
        i,
        ts[i],
        ts[i - 1]
      );
    }
  }
}
