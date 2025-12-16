//! Observable traits and type inference
//!
//! This module contains CoreObservable, Observable traits.
//!
//! The architecture has two layers:
//! 1. CoreObservable<O>: The pure logic kernel, generic over observer context
//! 2. Observable: User-facing trait with inferred Item/Err types via
//!    ObservableType

// Submodules
pub mod boxed;
pub mod connectable;
pub mod create;
pub mod defer;
pub mod from_fn;
pub mod from_future;
pub mod from_iter;
pub mod from_stream;
pub mod interval;
pub mod of;
pub mod timer;
pub mod trivial;

// Re-exports
// Standard library imports

pub use boxed::*;
pub use connectable::*;
pub use create::*;
pub use defer::*;
pub use from_fn::*;
pub use from_future::FromFuture;
pub use from_iter::*;
pub use from_stream::{FromStream, FromStreamResult};
pub use interval::*;
pub use of::*;
pub use timer::*;
pub use trivial::*;

// Internal imports (avoid circular dependency with prelude)
use crate::context::Context;
use crate::ops::{
  average::{Average, Averageable},
  buffer::Buffer, // Restored
  buffer_count::BufferCount,
  buffer_time::BufferTime,
  collect::Collect,
  combine_latest::CombineLatest,
  contains::Contains,
  debounce::Debounce,
  default_if_empty::DefaultIfEmpty,
  delay::{Delay, DelaySubscriptionOp},
  distinct::{Distinct, DistinctKey},
  distinct_until_changed::{DistinctUntilChanged, DistinctUntilKeyChanged},
  filter::Filter,
  filter_map::FilterMap,
  finalize::Finalize,
  group_by::GroupBy,
  into_future::{ObservableFuture, SupportsIntoFuture},
  into_stream::SupportsIntoStream,
  last::Last,
  lifecycle::{OnComplete, OnError},
  map::Map,
  map_err::MapErr,
  map_to::MapTo,
  merge::Merge,
  merge_all::MergeAll,
  observe_on::ObserveOn,
  pairwise::Pairwise,
  reduce::{Reduce, ReduceFn, ReduceInitialFn},
  retry::{Retry, RetryPolicy},
  sample::Sample,
  scan::Scan,
  skip::Skip,
  skip_last::SkipLast,
  skip_until::SkipUntil,
  skip_while::SkipWhile,
  start_with::StartWith,
  subscribe_on::SubscribeOn,
  switch_map::SwitchMap,
  take::Take,
  take_last::TakeLast,
  take_until::TakeUntil,
  take_while::TakeWhile,
  tap::Tap,
  throttle::{Throttle, ThrottleEdge, ThrottleWhenParam},
  with_latest_from::WithLatestFrom,
  zip::Zip,
};
use crate::{
  observer::FnMutObserver,
  scheduler::{Duration, Instant},
  subject::{Subject, SubjectPtr, SubjectPtrMutRef},
  subscription::Subscription,
};

/// The Public Type Trait
/// This trait purely exposes the types. It is what users see in error messages
/// and documentation.
pub trait ObservableType {
  type Item<'a>
  where
    Self: 'a;
  type Err;
}

/// CoreObservable: The publisher kernel
///
/// This trait represents the pure logic of an observable sequence.
/// It is generic over the observer context (O), which allows the same
/// logic to work with both Local and Shared contexts.
pub trait CoreObservable<O>: ObservableType {
  /// The subscription handle returned when subscribing
  type Unsub: Subscription;

  /// Subscribe an observer to this observable
  ///
  /// The observer parameter is typically a Context wrapping the actual
  /// observer.
  fn subscribe(self, observer: O) -> Self::Unsub;
}

/// Observable: The user-facing API
///
/// This trait extends Context and provides a clean API where users can see
/// the Item and Err types.
pub trait Observable: Context {
  /// The type of values emitted by this observable
  type Item<'a>
  where
    Self: 'a;

  /// The type of errors emitted by this observable
  type Err;

  /// Subscribe with a closure for the next values.
  ///
  /// This is the primary subscription method, optimized for ergonomics.
  fn subscribe<F, U>(self, f: F) -> U
  where
    F: for<'a> FnMut(Self::Item<'a>),
    Self::Inner: CoreObservable<Self::With<FnMutObserver<F>>, Unsub = U>,
  {
    let wrapped = Self::lift(FnMutObserver(f));
    self.into_inner().subscribe(wrapped)
  }

  /// Subscribe with a full Observer implementation.
  ///
  /// Use this method when you need to handle `error` and `complete` explicitly,
  /// or when subscribing a Subject/custom Observer struct.
  fn subscribe_with<O>(self, observer: O) -> <Self::Inner as CoreObservable<Self::With<O>>>::Unsub
  where
    Self::Inner: CoreObservable<Self::With<O>>,
  {
    let (core, wrapped) = self.swap(observer);
    core.subscribe(wrapped)
  }

  /// Apply a transformation to the values emitted by this observable
  ///
  /// This is a higher-order operator that returns a new observable
  /// with the transformed values.
  #[cfg(not(feature = "nightly"))]
  fn map<F, Out>(self, f: F) -> Self::With<Map<Self::Inner, F>>
  where
    F: for<'a> FnMut(Self::Item<'a>) -> Out,
  {
    self.transform(|core| Map { source: core, func: f })
  }

  #[cfg(feature = "nightly")]
  fn map<F>(self, f: F) -> Self::With<Map<Self::Inner, F>>
  where
    F: for<'a> FnMut<(Self::Item<'a>,)>,
  {
    self.transform(|core| Map { source: core, func: f })
  }

  /// Map every emission to a constant value
  ///
  /// This operator maps every emission from the source observable to a constant
  /// value. It requires the constant value to be `Clone` since it is emitted
  /// multiple times.
  fn map_to<B>(self, value: B) -> Self::With<MapTo<Self::Inner, B>>
  where
    B: Clone,
  {
    self.transform(|core| MapTo { source: core, value })
  }

  /// Filter items emitted by the source observable
  ///
  /// This operator emits only those items from the source Observable that pass
  /// a predicate test. Items that cause the predicate to return false are
  /// simply not emitted and are discarded.
  ///
  /// # Arguments
  ///
  /// * `filter` - A predicate function that takes a reference to an item and
  ///   returns true if it should be emitted
  ///
  /// # Type Parameters
  ///
  /// * `F` - The predicate function type
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).filter(|v| v % 2 == 0);
  /// // Emits: 2, 4
  /// ```
  fn filter<F>(self, filter: F) -> Self::With<Filter<Self::Inner, F>>
  where
    F: for<'a> FnMut(&Self::Item<'a>) -> bool,
  {
    self.transform(|source| Filter { source, filter })
  }

  /// Emit only items that have not been emitted before
  ///
  /// This operator filters out duplicate items, emitting only the first
  /// occurrence of each identical item. It uses a `HashSet` to keep track of
  /// seen items.
  ///
  /// # Requirements
  ///
  /// * `Item` must implement `Eq`, `Hash`, and `Clone`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 2, 3, 1, 3, 4]).distinct();
  /// // Emits: 1, 2, 3, 4
  /// ```
  fn distinct(self) -> Self::With<Distinct<Self::Inner>>
  where
    for<'a> Self::Item<'a>: Eq + std::hash::Hash + Clone,
  {
    self.transform(Distinct)
  }

  /// Emit only items where the key has not been emitted before
  ///
  /// This operator filters out items based on a key derived from each item.
  /// Only the first item with a specific key is emitted.
  ///
  /// # Arguments
  ///
  /// * `key_selector` - A function that extracts a key from each item used for
  ///   uniqueness
  ///
  /// # Requirements
  ///
  /// * `Key` must implement `Eq`, `Hash`, and `Clone`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([(1, "a"), (1, "b"), (2, "a")]).distinct_key(|(id, _)| *id);
  /// // Emits: (1, "a"), (2, "a")
  /// ```
  fn distinct_key<F, Key>(self, key_selector: F) -> Self::With<DistinctKey<Self::Inner, F>>
  where
    F: for<'a> Fn(&Self::Item<'a>) -> Key,
    Key: Eq + std::hash::Hash + Clone,
  {
    self.transform(|source| DistinctKey { source, key_selector })
  }

  /// Emit only items that are different from the previous item
  ///
  /// This operator filters out consecutive duplicate items.
  ///
  /// # Requirements
  ///
  /// * `Item` must implement `PartialEq` and `Clone`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 2, 3]).distinct_until_changed();
  /// // Emits: 1, 2, 3
  /// ```
  fn distinct_until_changed(self) -> Self::With<DistinctUntilChanged<Self::Inner>>
  where
    for<'a> Self::Item<'a>: PartialEq + Clone,
  {
    self.transform(DistinctUntilChanged)
  }

  /// Emit items which are different from the previous item based on a key
  /// selector
  ///
  /// This operator filters out consecutive items that have the same key.
  ///
  /// # Arguments
  ///
  /// * `key_selector` - A function that extracts a key from each item
  ///
  /// # Requirements
  ///
  /// * `Key` must implement `PartialEq`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable =
  ///   Local::from_iter([(1, 1), (1, 2), (2, 1)]).distinct_until_key_changed(|(k, _)| *k);
  /// // Emits: (1, 1), (2, 1)
  /// // Note: (1, 2) is skipped because its key (1) is same as previous (1)
  /// ```
  fn distinct_until_key_changed<F, Key>(
    self, key_selector: F,
  ) -> Self::With<DistinctUntilKeyChanged<Self::Inner, F>>
  where
    F: for<'a> Fn(&Self::Item<'a>) -> Key,
    Key: PartialEq,
  {
    self.transform(|source| DistinctUntilKeyChanged { source, key_selector })
  }

  /// Apply a function to each item and emit only the Some values
  ///
  /// This operator maps each item to an Option<T>, emitting only the Some(T)
  /// values. It effectively combines filtering and mapping in a single
  /// operation, allowing you to transform values while simultaneously
  /// filtering out None results.
  ///
  /// # Arguments
  ///
  /// * `f` - A function that takes an item and returns Option<Out>
  ///
  /// # Type Parameters
  ///
  /// * `Out` - The output type contained in the Option
  /// * `F` - The filter_map function type
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable =
  ///   Local::from_iter(["1", "2", "invalid", "3"]).filter_map(|s| s.parse::<i32>().ok());
  /// // Emits: 1, 2, 3
  /// ```
  fn filter_map<F, Out>(self, f: F) -> Self::With<FilterMap<Self::Inner, F>>
  where
    F: for<'a> FnMut(Self::Item<'a>) -> Option<Out>,
  {
    self.transform(|source| FilterMap { source, func: f })
  }

  /// Apply an accumulator function and emit each intermediate result
  ///
  /// This operator applies an accumulator function over the source Observable
  /// and returns each intermediate result. It's similar to `reduce` but emits
  /// the intermediate accumulations.
  ///
  /// # Arguments
  ///
  /// * `initial` - The initial accumulator value
  /// * `f` - A function that takes the current accumulator and a value, and
  ///   returns the new accumulator
  ///
  /// # Type Parameters
  ///
  /// * `Output` - The type of the accumulator value (must be Clone)
  /// * `F` - The accumulator function type
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4]).scan(0, |acc, v| acc + v);
  /// // Emits: 1, 3, 6, 10
  /// ```
  fn scan<Output, F>(self, initial: Output, f: F) -> Self::With<Scan<Self::Inner, F, Output>>
  where
    Output: Clone,
    F: for<'a> FnMut(Output, Self::Item<'a>) -> Output,
  {
    self.transform(|source| Scan { source, func: f, initial_value: initial })
  }

  /// Emit only the first `count` values from the source observable
  ///
  /// This operator emits only the first `count` values emitted by the source
  /// observable, then completes. If the source completes before emitting
  /// `count` values, `take` will also complete early.
  ///
  /// # Arguments
  ///
  /// * `count` - The maximum number of values to emit
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).take(3);
  /// // Emits: 1, 2, 3
  /// ```
  fn take(self, count: usize) -> Self::With<Take<Self::Inner>> {
    self.transform(|source| Take { source, count })
  }

  /// Emit only the first value from the source observable
  ///
  /// This operator emits only the first value emitted by the source
  /// observable, then completes. If the source completes without emitting
  /// any values, this operator will complete without emitting anything.
  ///
  /// This is equivalent to `take(1)`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3]).first();
  /// // Emits: 1
  /// ```
  fn first(self) -> Self::With<Take<Self::Inner>> { self.take(1) }

  /// Emit the first value from the source observable, or a default if empty
  ///
  /// This operator emits the first value that was emitted. If the source
  /// emits no values, it emits the provided default value instead.
  ///
  /// This is equivalent to `take(1).default_if_empty(default_value)`.
  ///
  /// # Arguments
  ///
  /// * `default_value` - The default value to emit if the source emits no
  ///   values
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// // With values - emits the first value
  /// let observable = Local::from_iter([1, 2, 3]).first_or(0);
  /// // Emits: 1
  ///
  /// // Without values - emits the default
  /// let observable = Local::from_iter(std::iter::empty::<i32>()).first_or(0);
  /// // Emits: 0
  /// ```
  fn first_or<'a>(
    self, default_value: Self::Item<'a>,
  ) -> Self::With<DefaultIfEmpty<Take<Self::Inner>, Self::Item<'a>>> {
    self.transform(|source| DefaultIfEmpty { source: Take { source, count: 1 }, default_value })
  }

  /// Skip the first `count` values from the source observable
  ///
  /// This operator ignores the first `count` values emitted by the source
  /// observable, then emits all subsequent values. If the source completes
  /// before emitting `count` values, `skip` will complete without emitting
  /// any values.
  ///
  /// # Arguments
  ///
  /// * `count` - The number of values to skip
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip(2);
  /// // Emits: 3, 4, 5
  /// ```
  fn skip(self, count: usize) -> Self::With<Skip<Self::Inner>> {
    self.transform(|source| Skip { source, count })
  }

  /// Emit only the last `count` values from the source observable
  ///
  /// This operator buffers the last `count` values emitted by the source
  /// observable. When the source completes, it emits all buffered values
  /// in order and then completes. If the source emits fewer than `count`
  /// values, all values are emitted.
  ///
  /// # Arguments
  ///
  /// * `count` - The maximum number of last values to emit
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).take_last(3);
  /// // Emits: 3, 4, 5
  /// ```
  fn take_last(self, count: usize) -> Self::With<TakeLast<Self::Inner>> {
    self.transform(|source| TakeLast { source, count })
  }

  /// Emit only the last value from the source observable
  ///
  /// This operator waits for the source to complete, then emits only the
  /// last value that was emitted. If the source emits no values, this operator
  /// will complete without emitting anything.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).last();
  /// // Emits: 5
  /// ```
  fn last(self) -> Self::With<Last<Self::Inner>> { self.transform(|source| Last { source }) }

  /// Emit the last value from the source observable, or a default if empty
  ///
  /// This operator waits for the source to complete, then emits the last value
  /// that was emitted. If the source emits no values, it emits the provided
  /// default value instead.
  ///
  /// This is equivalent to `last().default_if_empty(default_value)` but
  /// implemented as a single operator for better performance and convenience.
  ///
  /// # Arguments
  ///
  /// * `default_value` - The default value to emit if the source emits no
  ///   values
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// // With values - emits the last value
  /// let observable = Local::from_iter([1, 2, 3]).last_or(0);
  /// // Emits: 3
  ///
  /// // Without values - emits the default
  /// let observable = Local::from_iter(std::iter::empty::<i32>()).last_or(0);
  /// // Emits: 0
  /// ```
  fn last_or<'a>(
    self, default_value: Self::Item<'a>,
  ) -> Self::With<DefaultIfEmpty<Last<Self::Inner>, Self::Item<'a>>> {
    self.transform(|source| DefaultIfEmpty { source: Last { source }, default_value })
  }

  /// Skip the last `count` values from the source observable
  ///
  /// This operator buffers up to `count` values. When a new value arrives
  /// and the buffer is full, the oldest value is emitted. When the source
  /// completes, the buffered values (the last `count` items) are discarded.
  ///
  /// # Arguments
  ///
  /// * `count` - The number of last values to skip
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip_last(2);
  /// // Emits: 1, 2, 3
  /// ```
  fn skip_last(self, count: usize) -> Self::With<SkipLast<Self::Inner>> {
    self.transform(|source| SkipLast { source, count })
  }

  /// Skip values while a predicate returns true
  ///
  /// This operator skips values from the source observable as long as the
  /// predicate function returns true. Once the predicate returns false,
  /// it emits that value and all subsequent values.
  ///
  /// # Arguments
  ///
  /// * `predicate` - A function that takes a reference to an item and returns
  ///   true to skip
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).skip_while(|v| *v < 3);
  /// // Emits: 3, 4, 5
  /// ```
  fn skip_while<P>(self, predicate: P) -> Self::With<SkipWhile<Self::Inner, P>>
  where
    P: for<'a> FnMut(&Self::Item<'a>) -> bool,
  {
    self.transform(|source| SkipWhile { source, predicate })
  }

  /// Check if the source observable emits a specific value
  ///
  /// This operator emits `true` if the specified value is emitted by the source
  /// observable, otherwise it emits `false` when the source completes. The
  /// operation short-circuits and completes as soon as the target value is
  /// found.
  ///
  /// # Arguments
  ///
  /// * `target` - The value to search for (must implement PartialEq)
  ///
  /// # Requirements
  ///
  /// * `Item` must implement `PartialEq` and `Clone`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// // Check for existing value
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).contains(3);
  /// // Emits: true
  ///
  /// // Check for missing value
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).contains(10);
  /// // Emits: false
  /// ```
  fn contains<Item>(self, target: Item) -> Self::With<Contains<Self::Inner, Item>>
  where
    for<'a> Self::Item<'a>: PartialEq<Item>,
    Item: Clone,
  {
    self.transform(|source| Contains { source, target })
  }

  /// Emit values while a predicate returns true
  ///
  /// This operator emits values from the source observable as long as the
  /// predicate function returns true. When the predicate returns false,
  /// it completes the stream and stops emitting values.
  ///
  /// # Arguments
  ///
  /// * `predicate` - A function that takes a reference to an item and returns
  ///   true to continue emitting
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).take_while(|v| *v < 4);
  /// // Emits: 1, 2, 3
  /// ```
  fn take_while<P>(self, predicate: P) -> Self::With<TakeWhile<Self::Inner, P>>
  where
    P: for<'a> FnMut(&Self::Item<'a>) -> bool,
  {
    self.transform(|source| TakeWhile { source, predicate, inclusive: false })
  }

  /// Emit values while a predicate returns true, including the first failing
  /// value
  ///
  /// This operator emits values from the source observable as long as the
  /// predicate function returns true. When the predicate returns false,
  /// it emits that value (inclusive), then completes the stream.
  ///
  /// # Arguments
  ///
  /// * `predicate` - A function that takes a reference to an item and returns
  ///   true to continue emitting
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4, 5]).take_while_inclusive(|v| *v < 4);
  /// // Emits: 1, 2, 3, 4
  /// ```
  fn take_while_inclusive<P>(self, predicate: P) -> Self::With<TakeWhile<Self::Inner, P>>
  where
    P: for<'a> FnMut(&Self::Item<'a>) -> bool,
  {
    self.transform(|source| TakeWhile { source, predicate, inclusive: true })
  }

  /// Emit values until a notifier observable emits
  ///
  /// This operator emits values from the source observable until the notifier
  /// observable emits a value. When the notifier emits, the source
  /// subscription is cancelled and the stream completes.
  ///
  /// # Arguments
  ///
  /// * `notifier` - The observable that triggers termination
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let mut notifier = Local::subject();
  /// let source = Local::from_iter(0..10);
  /// source
  ///   .take_until(notifier.clone())
  ///   .subscribe(|v| println!("{}", v));
  /// notifier.next(());
  /// ```
  fn take_until<N>(self, notifier: N) -> Self::With<TakeUntil<Self::Inner, N::Inner>>
  where
    N: Observable<Err = Self::Err, Inner: ObservableType>,
  {
    self.transform(|source| TakeUntil { source, notifier: notifier.into_inner() })
  }

  /// Skip values until a notifier observable emits
  ///
  /// This operator skips values from the source observable until the notifier
  /// observable emits a value. Once the notifier emits, subsequent source
  /// values are passed through.
  ///
  /// # Arguments
  ///
  /// * `notifier` - The observable that triggers the start of emissions
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let mut notifier = Local::subject();
  /// let source = Local::from_iter(0..10);
  /// source
  ///   .skip_until(notifier.clone())
  ///   .subscribe(|v| println!("{}", v));
  /// notifier.next(());
  /// ```
  fn skip_until<N>(self, notifier: N) -> Self::With<SkipUntil<Self::Inner, N::Inner>>
  where
    N: Observable<Err = Self::Err, Inner: ObservableType>,
  {
    self.transform(|source| SkipUntil { source, notifier: notifier.into_inner() })
  }

  /// Group consecutive emissions into pairs
  ///
  /// This operator groups consecutive emissions from the source Observable into
  /// tuples `(previous, current)`. On the first emission, it stores the value.
  /// On subsequent emissions, it emits `(stored, current)` and updates the
  /// stored value.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let observable = Local::from_iter([1, 2, 3, 4]).pairwise();
  /// let mut result = Vec::new();
  /// observable.subscribe(|pair| result.push(pair));
  /// assert_eq!(result, vec![(1, 2), (2, 3), (3, 4)]);
  /// ```
  fn pairwise(self) -> Self::With<Pairwise<Self::Inner>> {
    self.transform(|source| Pairwise { source })
  }

  /// Merge this observable with another observable
  ///
  /// This operator combines two observables by subscribing to both and emitting
  /// values from either source as they arrive. The merged stream only completes
  /// when BOTH source streams complete.
  fn merge<'a, S2>(self, other: S2) -> Self::With<Merge<Self::Inner, S2::Inner>>
  where
    Self: 'a,
    S2: Observable<Inner: ObservableType<Item<'a> = Self::Item<'a>, Err = Self::Err>> + 'a,
  {
    self.transform(|core| Merge { source1: core, source2: other.into_inner() })
  }

  /// Combine the latest values from two observables
  ///
  /// This operator combines the latest values from two observables using a
  /// binary function. It emits whenever either source emits, using the most
  /// recent value from the other source.
  ///
  /// # Arguments
  ///
  /// * `other` - The second observable to combine with
  /// * `binary_op` - A function that combines the latest values from both
  ///   sources
  fn combine_latest<S2, F, OutputItem>(
    self, other: S2, binary_op: F,
  ) -> Self::With<CombineLatest<Self::Inner, S2::Inner, F>>
  where
    S2: Observable<Inner: ObservableType<Err = Self::Err>>,
    F: for<'a> FnMut(Self::Item<'a>, S2::Item<'a>) -> OutputItem,
  {
    self.transform(|core| CombineLatest { source_a: core, source_b: other.into_inner(), binary_op })
  }

  /// Combine each emission from this observable with the latest value from
  /// another
  ///
  /// Unlike `combine_latest`, this operator only emits when the primary (self)
  /// observable emits. The secondary observable just provides its latest
  /// value. If the secondary hasn't emitted yet, primary emissions are
  /// dropped.
  ///
  /// # Arguments
  ///
  /// * `other` - The secondary observable providing the latest value
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// let primary = Local::subject::<i32, Infallible>();
  /// let secondary = Local::subject::<i32, Infallible>();
  ///
  /// primary
  ///   .with_latest_from(secondary)
  ///   .subscribe(|(a, b): (i32, i32)| println!("({}, {})", a, b));
  /// ```
  fn with_latest_from<S2>(self, other: S2) -> Self::With<WithLatestFrom<Self::Inner, S2::Inner>>
  where
    S2: Observable<Inner: ObservableType<Err = Self::Err>>,
  {
    self.transform(|core| WithLatestFrom { source_a: core, source_b: other.into_inner() })
  }

  /// Flatten a Higher-Order Observable by merging inner observables
  ///
  /// This operator subscribes to inner observables as they arrive, up to
  /// the `concurrent` limit. When an inner observable completes, the next
  /// queued one is subscribed to.
  ///
  /// The stream completes when the outer observable AND all inner observables
  /// have completed.
  ///
  /// # Arguments
  ///
  /// * `concurrent` - Maximum number of inner observables to subscribe to
  ///   concurrently. Use `usize::MAX` for unlimited concurrency.
  fn merge_all(self, concurrent: usize) -> Self::With<MergeAll<Self::Inner>> {
    self.transform(|source| MergeAll { source, concurrent })
  }

  /// Flatten a Higher-Order Observable by subscribing to inner observables
  /// sequentially
  ///
  /// This is equivalent to `merge_all(1)` - waits for each inner observable to
  /// complete before subscribing to the next one.
  fn concat_all(self) -> Self::With<MergeAll<Self::Inner>> { self.merge_all(1) }

  /// Split the source observable into multiple GroupedObservables
  ///
  /// Each unique key derived from source values creates a new
  /// `GroupedObservable`. Values with the same key are forwarded to the
  /// corresponding group's observer.
  ///
  /// # Arguments
  ///
  /// * `key_selector` - A function that extracts a key from each item
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter(0..10)
  ///   .group_by(|v| *v % 2 == 0)
  ///   .subscribe(|group| {
  ///     if group.inner.key {
  ///       group.subscribe(|v| println!("Even: {}", v));
  ///     } else {
  ///       group.subscribe(|v| println!("Odd: {}", v));
  ///     }
  ///   });
  /// ```
  #[allow(clippy::type_complexity)]
  fn group_by<'a, F, Key>(
    self, key_selector: F,
  ) -> Self::With<
    GroupBy<Self::Inner, F, Self::With<Subject<SubjectPtr<'a, Self, Self::Item<'a>, Self::Err>>>>,
  >
  where
    F: FnMut(&Self::Item<'a>) -> Key,
    Key: std::hash::Hash + Eq + Clone,
  {
    self.transform(|source| GroupBy::new(source, key_selector))
  }

  /// Split the source observable into multiple GroupedObservables for mutable
  /// references
  ///
  /// Each unique key derived from source values creates a new
  /// `GroupedObservable`. Values with the same key are forwarded to the
  /// corresponding group's observer. The values are mutable references,
  /// allowing modification.
  ///
  /// # Arguments
  ///
  /// * `key_selector` - A function that extracts a key from each item
  #[allow(clippy::type_complexity)]
  fn group_by_mut_ref<'a, F, Key, Item: 'a>(
    self, key_selector: F,
  ) -> Self::With<
    GroupBy<Self::Inner, F, Self::With<Subject<SubjectPtrMutRef<'a, Self, Item, Self::Err>>>>,
  >
  where
    Self: Observable<Item<'a> = &'a mut Item> + 'a,
    F: FnMut(&Self::Item<'a>) -> Key,
    Key: std::hash::Hash + Eq + Clone,
  {
    self.transform(|source| GroupBy::new(source, key_selector))
  }

  /// Execute a callback when an error occurs
  ///
  /// This operator allows intercepting errors for side effects while preserving
  /// the original error flow downstream.
  fn on_error<F>(self, f: F) -> Self::With<OnError<Self::Inner, F>>
  where
    F: FnOnce(Self::Err),
  {
    self.transform(|core| OnError::new(core, f))
  }

  /// Transform the error emitted by the source observable
  ///
  /// This operator intercepts an error emission from the source and maps it to
  /// a new error value/type using the provided function.
  ///
  /// # Arguments
  ///
  /// * `f` - A function that takes the original error and returns the new error
  ///
  /// # Type Parameters
  ///
  /// * `OutErr` - The new error type
  /// * `F` - The mapping function type
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::{cell::RefCell, rc::Rc};
  ///
  /// use rxrust::prelude::*;
  ///
  /// let got = Rc::new(RefCell::new(None));
  /// let got_clone = got.clone();
  ///
  /// Local::throw_err("oops")
  ///   .map_err(|e| format!("Error: {}", e))
  ///   .on_error(move |e| *got_clone.borrow_mut() = Some(e))
  ///   .subscribe(|_| {});
  ///
  /// assert_eq!(*got.borrow(), Some("Error: oops".to_string()));
  /// ```
  fn map_err<F, OutErr>(self, f: F) -> Self::With<MapErr<Self::Inner, F>>
  where
    F: FnOnce(Self::Err) -> OutErr,
  {
    self.transform(|source| MapErr { source, func: f })
  }

  /// Execute a callback when the stream completes
  ///
  /// This operator allows reacting to completion events for side effects.
  fn on_complete<F>(self, f: F) -> Self::With<OnComplete<Self::Inner, F>>
  where
    F: FnOnce(),
  {
    self.transform(|core| OnComplete::new(core, f))
  }

  /// Delay emissions by the specified duration
  ///
  /// This operator delays each emission by the specified duration using the
  /// context's scheduler. Error and completion notifications are NOT delayed.
  fn delay(self, delay: Duration) -> Self::With<Delay<Self::Inner, Self::Scheduler>> {
    let scheduler = self.scheduler().clone();
    self.transform(|core| Delay { source: core, delay, scheduler })
  }

  /// Delay emissions until the specified instant
  ///
  /// This operator delays each emission until the specified instant using the
  /// context's scheduler. If the instant is in the past, emissions happen
  /// immediately.
  fn delay_at(self, instant: Instant) -> Self::With<Delay<Self::Inner, Self::Scheduler>> {
    let now = Instant::now();
    let delay = if instant <= now { Duration::ZERO } else { instant - now };
    self.delay(delay)
  }

  /// Delay emissions by a specified duration using a custom scheduler
  ///
  /// This operator delays each emission by the specified duration using the
  /// provided scheduler, overriding the context's default scheduler.
  fn delay_with<Sch>(self, delay: Duration, scheduler: Sch) -> Self::With<Delay<Self::Inner, Sch>> {
    self.transform(|core| Delay { source: core, delay, scheduler })
  }

  /// Delay the subscription to the source observable by the specified duration
  ///
  /// This operator timeshifts the moment of subscription to the source
  /// Observable.
  fn delay_subscription(
    self, delay: Duration,
  ) -> Self::With<DelaySubscriptionOp<Self::Inner, Self::Scheduler>> {
    let scheduler = self.scheduler().clone();
    self.transform(|core| DelaySubscriptionOp { source: core, delay, scheduler })
  }

  /// Delay the subscription to the source observable until the specified
  /// instant
  fn delay_subscription_at(
    self, instant: Instant,
  ) -> Self::With<DelaySubscriptionOp<Self::Inner, Self::Scheduler>> {
    let now = Instant::now();
    let delay = if instant <= now { Duration::ZERO } else { instant - now };
    self.delay_subscription(delay)
  }

  /// Delay the subscription to the source observable by the specified duration
  /// using a custom scheduler
  fn delay_subscription_with<Sch>(
    self, delay: Duration, scheduler: Sch,
  ) -> Self::With<DelaySubscriptionOp<Self::Inner, Sch>> {
    self.transform(|core| DelaySubscriptionOp { source: core, delay, scheduler })
  }

  /// Emit a value only after a quiet period has passed
  ///
  /// Emits an item from the source Observable only after a particular duration
  /// has passed without another source emission. Each new emission resets the
  /// timer.
  ///
  /// # Arguments
  ///
  /// * `duration` - The quiet period duration after which to emit the last
  ///   value
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// // Only emit after 100ms of no emissions
  /// let source = Local::from_iter([1, 2, 3, 4, 5]);
  /// source
  ///   .debounce(Duration::from_millis(100))
  ///   .subscribe(|v| println!("Debounced: {}", v));
  /// ```
  fn debounce(self, duration: Duration) -> Self::With<Debounce<Self::Inner, Self::Scheduler>> {
    let scheduler = self.scheduler().clone();
    self.transform(|core| Debounce { source: core, duration, scheduler })
  }

  /// Emit a value only after a quiet period has passed, using a custom
  /// scheduler
  ///
  /// This variant allows specifying a custom scheduler for the debounce timer.
  fn debounce_with<Sch>(
    self, duration: Duration, scheduler: Sch,
  ) -> Self::With<Debounce<Self::Inner, Sch>> {
    self.transform(|core| Debounce { source: core, duration, scheduler })
  }

  /// Re-emits all notifications from this Observable on the specified
  /// Scheduler.
  ///
  /// This operator ensures that the downstream observer's methods (`next`,
  /// `error`, `complete`) are executed in the context of the provided
  /// scheduler.
  ///
  /// # Arguments
  ///
  /// * `scheduler` - The scheduler to use for emitting notifications
  fn observe_on<Sch>(self, scheduler: Sch) -> Self::With<ObserveOn<Self::Inner, Sch>> {
    self.transform(|core| ObserveOn { source: core, scheduler })
  }

  /// Schedule the subscription (and work done during subscription) on a
  /// specified Scheduler.
  ///
  /// Unlike `observe_on` which affects where notifications are delivered,
  /// `subscribe_on` affects where the source observable's subscription logic
  /// runs.
  ///
  /// # Arguments
  ///
  /// * `scheduler` - The scheduler on which to schedule the subscription
  fn subscribe_on<Sch>(self, scheduler: Sch) -> Self::With<SubscribeOn<Self::Inner, Sch>> {
    self.transform(|core| SubscribeOn { source: core, scheduler })
  }

  /// Resubscribe to the source observable when it errors
  ///
  /// This operator resubscribes to the source observable when it emits an
  /// error. It can be used to retry failed operations.
  ///
  /// # Arguments
  ///
  /// * `policy` - The retry policy. This can be:
  ///   - `usize`: A simple retry count (e.g., `.retry(3)`).
  ///   - [`RetryConfig`]: A configuration object for advanced control (delay,
  ///     reset on success).
  ///   - A custom type implementing the [`RetryPolicy`] trait.
  ///
  /// # Examples
  ///
  /// Simple retry with count:
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// Local::throw_err("fail")
  ///   .retry(3)
  ///   .on_error(|_e| {})
  ///   .subscribe(|_| {});
  /// ```
  ///
  /// Advanced retry with configuration:
  ///
  /// ```rust,no_run
  /// use rxrust::{ops::RetryConfig, prelude::*};
  ///
  /// Local::throw_err("fail")
  ///   .retry(
  ///     RetryConfig::new()
  ///       .count(3)
  ///       .delay(Duration::from_millis(100)),
  ///   )
  ///   .on_error(|_e| {})
  ///   .subscribe(|_| {});
  /// ```
  ///
  /// Custom retry policy (e.g., HTTP status code):
  ///
  /// ```rust,no_run
  /// use rxrust::{ops::retry::RetryPolicy, prelude::*};
  ///
  /// #[derive(Clone)]
  /// struct HttpRetry;
  ///
  /// impl RetryPolicy<u16> for HttpRetry {
  ///   fn should_retry(&self, err: &u16, attempt: usize) -> Option<Duration> {
  ///     match err {
  ///       500 => Some(Duration::from_millis(100)), // Retry server errors
  ///       404 => None,                             // Do not retry not found
  ///       _ => None,
  ///     }
  ///   }
  /// }
  ///
  /// Local::throw_err(500u16)
  ///   .retry(HttpRetry)
  ///   .on_error(|_e| {})
  ///   .subscribe(|_| {});
  /// ```
  fn retry<P>(self, policy: P) -> Self::With<Retry<Self::Inner, P, Self::Scheduler>>
  where
    P: RetryPolicy<Self::Err>,
  {
    let scheduler = self.scheduler().clone();
    self.transform(|source| Retry { source, policy, scheduler })
  }

  /// Throttle emissions by ignoring values during a window
  ///
  /// Each emitted value starts a throttle window. While the
  /// window is active, additional source values are suppressed.
  ///
  /// The window is closed by a notifier Observable returned by `selector`.
  /// The first `next` (or `complete`) from the notifier ends the window.
  ///
  /// If `edge.trailing` is enabled, the last suppressed value is emitted when
  /// the window ends. If a trailing value is emitted, it starts a new window.
  ///
  /// Completion semantics: if the source completes while a window is active
  /// and a trailing value is pending, completion is delayed until the window
  /// ends and the trailing value is emitted.
  ///
  /// # Arguments
  ///
  /// * `selector` - A function that takes a reference to an item and returns a
  ///   notifier Observable that closes the throttle window
  /// * `edge` - Configuration for leading/trailing behavior
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// // Throttle with a dynamic notifier based on value
  /// let source = Local::from_iter([1u64, 2, 3, 4, 5]);
  /// source
  ///   .throttle(|v| Local::timer(Duration::from_millis(*v * 10)), ThrottleEdge::leading())
  ///   .subscribe(|v| println!("Throttled: {}", v));
  /// ```
  fn throttle<F, Out>(
    self, selector: F, edge: ThrottleEdge,
  ) -> Self::With<Throttle<Self::Inner, ThrottleWhenParam<F>>>
  where
    F: for<'a> FnMut(&Self::Item<'a>) -> Out,
    Out: Observable<Err = Self::Err>,
  {
    self.transform(|core| Throttle { source: core, param: ThrottleWhenParam { selector }, edge })
  }

  /// Throttle emissions with a fixed duration
  ///
  /// Time-based convenience wrapper around notifier-based throttle.
  ///
  /// # Arguments
  ///
  /// * `duration` - The throttle window duration
  /// * `edge` - Configuration for leading/trailing behavior
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// // Emit first value, then ignore for 100ms
  /// let source = Local::from_iter([1, 2, 3, 4, 5]);
  /// source
  ///   .throttle_time(Duration::from_millis(100), ThrottleEdge::leading())
  ///   .subscribe(|v| println!("Throttled: {}", v));
  /// ```
  fn throttle_time(
    self, duration: Duration, edge: ThrottleEdge,
  ) -> Self::With<Throttle<Self::Inner, Self::With<Duration>>>
  where
    Self::With<Duration>: Context<Scheduler = Self::Scheduler>,
  {
    let scheduler = self.scheduler().clone();
    self.throttle_time_with(duration, edge, scheduler)
  }

  /// Throttle emissions with a fixed duration using a custom scheduler
  ///
  /// This variant allows specifying a custom scheduler for the throttle timer.
  fn throttle_time_with<Sch>(
    self, duration: Duration, edge: ThrottleEdge, scheduler: Sch,
  ) -> Self::With<Throttle<Self::Inner, Self::With<Duration>>>
  where
    Self::With<Duration>: Context<Scheduler = Sch>,
  {
    self.transform(move |core| Throttle {
      source: core,
      param: Self::With::from_parts(duration, scheduler),
      edge,
    })
  }

  /// Emit the most recently emitted value when a sampler Observable emits
  ///
  /// Whenever the sampler Observable emits a value, emit the most recently
  /// emitted value from the source Observable. Also emits the stored value
  /// when the sampler completes.
  ///
  /// # Arguments
  ///
  /// * `sampler` - The observable that controls when to sample the source
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let mut source = Local::subject();
  /// let mut sampler = Local::subject();
  ///
  /// let source_sub = source.clone();
  /// let sampler_sub = sampler.clone();
  ///
  /// source_sub
  ///   .sample(sampler_sub)
  ///   .subscribe(|v| println!("Sampled: {}", v));
  ///
  /// source.next(1);
  /// source.next(2);
  /// sampler.next(()); // Emits 2
  /// source.next(3);
  /// sampler.next(()); // Emits 3
  /// ```
  fn sample<N>(self, sampler: N) -> Self::With<Sample<Self::Inner, N::Inner>>
  where
    N: Observable<Err = Self::Err, Inner: ObservableType>,
  {
    self.transform(|source| Sample { source, sampler: sampler.into_inner() })
  }

  /// Combine items from two observables pairwise
  ///
  /// Zip combines items from this observable with items from another observable
  /// pairwise. It buffers items from each source and emits a tuple `(ItemA,
  /// ItemB)` when both sources have emitted a value. The stream completes
  /// when both sources complete.
  ///
  /// # Arguments
  ///
  /// * `other` - The second observable to zip with
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([1, 2, 3])
  ///   .zip(Local::from_iter([4, 5, 6]))
  ///   .subscribe(|(a, b)| println!("({}, {})", a, b));
  /// // Prints: (1, 4), (2, 5), (3, 6)
  /// ```
  fn zip<B>(self, other: B) -> Self::With<Zip<Self::Inner, B::Inner>>
  where
    B: Observable<Err = Self::Err, Inner: ObservableType>,
  {
    self.transform(|source_a| Zip { source_a, source_b: other.into_inner() })
  }

  /// Perform a side effect for each emission
  ///
  /// The `tap` operator allows you to perform side effects for each value
  /// emitted by the source observable, without modifying the values themselves.
  /// It's useful for debugging, logging, or triggering external actions.
  ///
  /// # Arguments
  ///
  /// * `f` - A function that takes a reference to each emitted value
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([1, 2, 3])
  ///   .tap(|v| println!("Got value: {}", v))
  ///   .subscribe(|_| {});
  /// ```
  fn tap<F>(self, f: F) -> Self::With<Tap<Self::Inner, F>>
  where
    F: for<'a> FnMut(&Self::Item<'a>),
  {
    self.transform(|source| Tap { source, func: f })
  }

  /// Execute cleanup logic when the Observable terminates
  ///
  /// The `finalize` operator calls a function when the Observable completes,
  /// errors, or is unsubscribed. The function is guaranteed to be called
  /// exactly once, regardless of how the Observable terminates.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::{cell::Cell, rc::Rc};
  ///
  /// use rxrust::prelude::*;
  ///
  /// let finalized = Rc::new(Cell::new(false));
  /// let finalized_clone = finalized.clone();
  ///
  /// Local::of(1)
  ///   .finalize(move || finalized_clone.set(true))
  ///   .subscribe(|_| {});
  ///
  /// assert!(finalized.get());
  /// ```
  fn finalize<F>(self, f: F) -> Self::With<Finalize<Self::Inner, F>>
  where
    F: FnOnce(),
  {
    self.transform(|source| Finalize { source, func: f })
  }

  /// Emit specified values before beginning to emit source values
  ///
  /// The `start_with` operator prepends the provided values to the source
  /// Observable. The values are emitted synchronously before subscribing
  /// to the source.
  ///
  /// # Arguments
  ///
  /// * `values` - A Vec of items to emit before the source items
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([3, 4, 5])
  ///   .start_with(vec![1, 2])
  ///   .subscribe(|v| println!("{}", v));
  /// // Prints: 1, 2, 3, 4, 5
  /// ```
  fn start_with<Item>(self, values: Vec<Item>) -> Self::With<StartWith<Self::Inner, Item>> {
    self.transform(|source| StartWith { source, values })
  }

  /// Emit a default value if the observable completes without emitting any
  /// items
  ///
  /// The `default_if_empty` operator emits a default value if the source
  /// Observable completes without emitting any items. If the source emits
  /// any items, the default value is not emitted.
  ///
  /// # Arguments
  ///
  /// * `default_value` - The value to emit if the source is empty
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::empty()
  ///   .map_to(0)
  ///   .default_if_empty(42)
  ///   .subscribe(|v| println!("{}", v));
  /// // Prints: 42
  /// ```
  fn default_if_empty<'a>(
    self, default_value: Self::Item<'a>,
  ) -> Self::With<DefaultIfEmpty<Self::Inner, Self::Item<'a>>> {
    self.transform(|source| DefaultIfEmpty::new(source, default_value))
  }

  /// Collect all emitted items into a collection
  ///
  /// The `collect` operator accumulates all items emitted by the source
  /// Observable into a collection. When the source completes, the collection
  /// is emitted as a single value.
  ///
  /// If the source errors, the error is propagated without emitting the
  /// collection.
  ///
  /// # Type Parameters
  ///
  /// * `C` - The collection type, must implement `Default` and `Extend<Item>`
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([1, 2, 3])
  ///   .collect::<Vec<_>>()
  ///   .subscribe(|v| println!("{:?}", v));
  /// // Prints: [1, 2, 3]
  /// ```
  fn collect<C>(self) -> Self::With<Collect<Self::Inner, C>>
  where
    C: Default,
  {
    self.collect_into(C::default())
  }

  /// Collect all emitted items into an existing collection
  ///
  /// Similar to `collect`, but starts with a pre-populated collection.
  /// New items are appended to the provided initial collection.
  ///
  /// # Arguments
  ///
  /// * `initial` - The initial collection to start with
  ///
  /// # Type Parameters
  ///
  /// * `C` - The collection type, must implement `Extend<Item>`
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([4, 5, 6])
  ///   .collect_into::<Vec<_>>(vec![1, 2, 3])
  ///   .subscribe(|v| println!("{:?}", v));
  /// // Prints: [1, 2, 3, 4, 5, 6]
  /// ```
  fn collect_into<C>(self, initial: C) -> Self::With<Collect<Self::Inner, C>> {
    self.transform(|source| Collect { source, collection: initial })
  }

  /// Buffer items until a notifier observable emits
  ///
  /// Collects items from the source observable into a Vec. When the notifier
  /// emits, the buffered items are emitted as a Vec and the buffer is cleared.
  /// When the source completes, any remaining items are emitted.
  ///
  /// # Arguments
  ///
  /// * `notifier` - The observable that triggers buffer emission
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// let notifier = Local::subject::<(), Infallible>();
  /// let source = Local::subject::<i32, Infallible>();
  ///
  /// source
  ///   .buffer(notifier)
  ///   .subscribe(|v| println!("{:?}", v));
  /// ```
  fn buffer<N>(self, notifier: N) -> Self::With<Buffer<Self::Inner, N::Inner>>
  where
    N: Observable<Err = Self::Err, Inner: ObservableType>,
  {
    self.transform(|source| Buffer { source, notifier: notifier.into_inner() })
  }

  /// Buffer items until a count is reached
  ///
  /// Collects items from the source observable into a Vec. When the count
  /// is reached, the buffered items are emitted as a Vec and the buffer is
  /// cleared. When the source completes, any remaining items are emitted.
  ///
  /// # Arguments
  ///
  /// * `count` - The number of items to buffer before emitting
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::from_iter([1, 2, 3, 4, 5])
  ///   .buffer_count(2)
  ///   .subscribe(|v| println!("{:?}", v));
  /// // Prints: [1, 2], [3, 4], [5]
  /// ```
  fn buffer_count(self, count: usize) -> Self::With<BufferCount<Self::Inner>> {
    self.transform(|source| BufferCount { source, count })
  }

  /// Buffer items for a time window
  ///
  /// Collects items from the source observable into a Vec. When the time
  /// window expires, the buffered items are emitted as a Vec and a new
  /// window starts. When the source completes, any remaining items are emitted.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::from_iter([1, 2, 3, 4, 5]);
  /// source
  ///   .buffer_time(Duration::from_millis(100))
  ///   .subscribe(|v| println!("{:?}", v));
  /// ```
  fn buffer_time(self, duration: Duration) -> Self::With<BufferTime<Self::Inner, Self::Scheduler>> {
    let scheduler = self.scheduler().clone();
    self.transform(|source| BufferTime { source, duration, max_buffer_size: None, scheduler })
  }

  /// Buffer items until time OR max count is reached
  ///
  /// The buffer is emitted when either the time window expires or the max
  /// buffer size is reached, whichever comes first.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::from_iter([1, 2, 3, 4, 5]);
  /// source
  ///   .buffer_time_max(Duration::from_millis(100), 10)
  ///   .subscribe(|v| println!("{:?}", v));
  /// ```
  fn buffer_time_max(
    self, duration: Duration, max_buffer_size: usize,
  ) -> Self::With<BufferTime<Self::Inner, Self::Scheduler>> {
    let scheduler = self.scheduler().clone();
    self.transform(|source| BufferTime {
      source,
      duration,
      max_buffer_size: Some(max_buffer_size),
      scheduler,
    })
  }

  /// Buffers items emitted by the source observable for a specified time
  /// window, using a custom scheduler for timing control.
  ///
  /// When the time window expires, the buffered items are emitted as a `Vec<T>`
  /// and a new buffer window starts. This variant allows you to provide your
  /// own scheduler for precise timing control (e.g., `TestScheduler` for
  /// testing).
  ///
  /// # Arguments
  /// * `duration` - The time span for each buffer window
  /// * `scheduler` - The scheduler to use for timing the buffer windows
  ///
  /// # Example
  /// ```ignore
  /// observable
  ///   .buffer_time_with(Duration::from_millis(100), my_scheduler)
  ///   .subscribe(|buffer| println!("Buffered: {:?}", buffer));
  /// ```
  ///
  /// # See Also
  /// * [`buffer_time`] - Uses the default scheduler
  /// * [`buffer_time_max_with`] - Adds a max buffer size constraint
  fn buffer_time_with<Sch>(
    self, duration: Duration, scheduler: Sch,
  ) -> Self::With<BufferTime<Self::Inner, Sch>> {
    self.transform(|source| BufferTime { source, duration, max_buffer_size: None, scheduler })
  }

  /// Buffers items emitted by the source observable until either the time
  /// window expires OR the maximum buffer size is reached, whichever comes
  /// first. Uses a custom scheduler for timing control.
  ///
  /// This is useful when you want to limit memory usage by capping the buffer
  /// size while still ensuring timely delivery of smaller batches.
  ///
  /// # Arguments
  /// * `duration` - The maximum time span for each buffer window
  /// * `max_buffer_size` - The maximum number of items per buffer; when
  ///   reached, the buffer is emitted immediately and a new window starts
  /// * `scheduler` - The scheduler to use for timing the buffer windows
  ///
  /// # Example
  /// ```ignore
  /// // Emit buffer every 100ms OR when 5 items are collected
  /// observable
  ///   .buffer_time_max_with(Duration::from_millis(100), 5, my_scheduler)
  ///   .subscribe(|buffer| println!("Buffered: {:?}", buffer));
  /// ```
  ///
  /// # See Also
  /// * [`buffer_time_max`] - Uses the default scheduler
  /// * [`buffer_time_with`] - Without max buffer size constraint
  fn buffer_time_max_with<Sch>(
    self, duration: Duration, max_buffer_size: usize, scheduler: Sch,
  ) -> Self::With<BufferTime<Self::Inner, Sch>> {
    self.transform(|source| BufferTime {
      source,
      duration,
      max_buffer_size: Some(max_buffer_size),
      scheduler,
    })
  }

  /// Convert this observable into a ConnectableObservable using the specified
  /// subject
  ///
  /// This operator creates a ConnectableObservable that will multicast values
  /// from this observable through the provided subject. The connection must
  /// be established explicitly via the `connect()` method on the resulting
  /// ConnectableObservable.
  ///
  /// # Multicasting Behavior
  ///
  /// - The source observable is subscribed to only once when `connect()` is
  ///   called
  /// - Values are broadcast to all subscribed observers
  /// - Requires `Item: Clone` for value broadcasting
  /// - Multiple observers share the same subscription to the source
  ///
  /// # Arguments
  ///
  /// * `subject` - The subject that will multicast values to observers
  ///
  /// # Returns
  ///
  /// A ConnectableObservable wrapped in the same Context. Use `fork()` to
  /// create Observable instances and `connect()` to start multicasting.
  ///
  /// # Example
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::of(1).merge(Local::of(2));
  /// let subject = Local::subject();
  /// let connectable = source.multicast(subject.into_inner());
  ///
  /// // Subscribe multiple observers (they will receive cloned values)
  /// let obs1 = connectable.fork();
  /// let obs2 = connectable.fork();
  /// obs1.subscribe(|v| println!("Observer 1: {}", v));
  /// obs2.subscribe(|v| println!("Observer 2: {}", v));
  ///
  /// // Start multicasting - source is subscribed to once
  /// let connection = connectable.connect();
  ///
  /// // Later, disconnect to stop multicasting
  /// connection.unsubscribe();
  /// ```
  fn multicast<'a>(
    self, subject: Subject<SubjectPtr<'a, Self, Self::Item<'a>, Self::Err>>,
  ) -> ConnectableObservableCtx<'a, Self> {
    self.transform(|source| ConnectableObservable { source, subject })
  }

  /// Convert this observable into a ConnectableObservable using the specified
  /// subject for mutable reference broadcasting
  ///
  /// This operator creates a ConnectableObservable that will multicast mutable
  /// references from this observable through the provided subject. The
  /// connection must be established explicitly via the `connect()` method on
  /// the resulting ConnectableObservable.
  ///
  /// # Mutable Reference Broadcasting
  ///
  /// - Uses Higher-Rank Trait Bounds (HRTB) to allow sequential modification
  /// - No `Clone` requirement on `Item` type
  /// - Observers receive `&mut Item` and can modify values
  /// - Multiple observers can modify the same value sequentially
  /// - Source observable is subscribed to only once when `connect()` is called
  ///
  /// # Arguments
  ///
  /// * `subject` - The subject that will multicast mutable references to
  ///   observers
  ///
  /// # Returns
  ///
  /// A ConnectableObservable wrapped in the same Context.
  ///
  /// # Example
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::subject_mut_ref();
  /// let subject = Local::subject_mut_ref();
  /// let connectable = source.multicast_mut_ref(subject.into_inner());
  ///
  /// // Subscribe multiple observers that can modify values sequentially
  /// connectable
  ///   .fork()
  ///   .subscribe(|v: &mut i32| *v += 1);
  /// connectable
  ///   .fork()
  ///   .subscribe(|v: &mut i32| *v *= 2);
  ///
  /// // Start multicasting - source is subscribed to once
  /// let connection = connectable.connect();
  ///
  /// // Later, disconnect to stop multicasting
  /// connection.unsubscribe();
  /// ```
  #[allow(clippy::type_complexity)]
  fn multicast_mut_ref<'a, Item: 'a>(
    self, subject: Subject<SubjectPtrMutRef<'a, Self, Item, Self::Err>>,
  ) -> ConnectableObservableCtxMutRef<'a, Self, Item>
  where
    Self: Observable<Item<'a> = &'a mut Item> + 'a,
  {
    self.transform(|source| ConnectableObservable { source, subject })
  }

  /// Convert this observable into a ConnectableObservable for value
  /// broadcasting
  ///
  /// This operator creates a ConnectableObservable that will multicast values
  /// from this observable through a subject. The connection must
  /// be established explicitly via the `connect()` method on the resulting
  /// ConnectableObservable.
  ///
  /// This is equivalent to `multicast(Subject::default())` and provides
  /// a convenient shorthand for creating a standard hot observable.
  ///
  /// # Value Broadcasting Behavior
  ///
  /// - New subscribers will only receive values emitted **after** they
  ///   subscribe
  /// - No historical values are replayed
  /// - Multiple observers share the same subscription to the source
  /// - Requires `Item: Clone` for broadcasting values to multiple observers
  /// - Source observable is subscribed to only once when `connect()` is called
  ///
  /// # Returns
  ///
  /// A ConnectableObservable wrapped in the same Context.
  ///
  /// # Example
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::of(1).merge(Local::of(2));
  /// let connectable = source.publish();
  ///
  /// // Subscribe multiple observers
  /// let obs1 = connectable.fork();
  /// let obs2 = connectable.fork();
  /// obs1.subscribe(|v| println!("Observer 1: {}", v));
  /// obs2.subscribe(|v| println!("Observer 2: {}", v));
  ///
  /// // Start multicasting - source is subscribed to once
  /// let connection = connectable.connect();
  ///
  /// // Later, disconnect to stop multicasting
  /// connection.unsubscribe();
  /// ```
  fn publish<'a>(self) -> ConnectableObservableCtx<'a, Self> { self.multicast(Subject::default()) }

  /// Convert this observable into a ConnectableObservable for mutable reference
  /// broadcasting
  ///
  /// This operator creates a ConnectableObservable that will multicast mutable
  /// references from this observable through a subject. The connection must
  /// be established explicitly via the `connect()` method on the resulting
  /// ConnectableObservable.
  ///
  /// This is equivalent to `multicast_mut_ref(Subject::default())` and provides
  /// a convenient shorthand for creating a standard hot observable that
  /// supports sequential modification via re-borrowing.
  ///
  /// # Mutable Reference Broadcasting Behavior
  ///
  /// - Uses Higher-Rank Trait Bounds (HRTB) to allow sequential modification
  /// - Subscribers receive `&mut Item` and can modify values
  /// - Multiple observers can modify the same value sequentially
  /// - No `Clone` requirement on `Item` type
  /// - No historical values are replayed
  /// - Source observable is subscribed to only once when `connect()` is called
  ///
  /// # Returns
  ///
  /// A ConnectableObservable wrapped in the same Context.
  ///
  /// # Example
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let source = Local::subject_mut_ref();
  /// let connectable = source.publish_mut_ref();
  ///
  /// // Subscribe multiple observers that can modify values sequentially
  /// let obs1 = connectable.fork();
  /// let obs2 = connectable.fork();
  /// obs1.subscribe(|v: &mut i32| *v += 1);
  /// obs2.subscribe(|v: &mut i32| *v *= 2);
  ///
  /// // Start multicasting - source is subscribed to once
  /// let connection = connectable.connect();
  ///
  /// // Later, disconnect to stop multicasting
  /// connection.unsubscribe();
  /// ```
  #[allow(clippy::type_complexity)]
  fn publish_mut_ref<'a, Item: 'a>(
    self,
  ) -> Self::With<ConnectableObservable<Self::Inner, SubjectPtrMutRef<'a, Self, Item, Self::Err>>>
  where
    Self: Observable<Item<'a> = &'a mut Item> + 'a,
  {
    self.multicast_mut_ref(Subject::default())
  }

  /// Type-erases a local observable into a boxed observable.
  ///
  /// This enables storing observables in collections or returning them from
  /// functions without complex generic types. All type information except
  /// `Item` and `Err` is erased.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// // Different source types become the same boxed type
  /// let boxed1 = Local::of(1).box_it();
  /// let boxed2 = Local::from_iter([2, 3]).map(|x| x * 2).box_it();
  ///
  /// // Store in a collection
  /// let observables = vec![boxed1, boxed2];
  /// ```
  fn box_it<'a>(self) -> Self::With<Self::BoxedCoreObservable<'a, Self::Item<'a>, Self::Err>>
  where
    Self::Inner: IntoBoxedCoreObservable<Self::BoxedCoreObservable<'a, Self::Item<'a>, Self::Err>>,
  {
    self.transform(|inner| inner.into_boxed())
  }

  /// Type-erases a local/shared observable into a *cloneable* boxed observable.
  ///
  /// This is only available when the underlying observable pipeline is
  /// `Clone` (enforced via trait bounds on the boxing target).
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let boxed = Local::of(42).box_it_clone();
  /// let boxed2 = boxed.clone();
  ///
  /// boxed.subscribe(|v| assert_eq!(v, 42));
  /// boxed2.subscribe(|v| assert_eq!(v, 42));
  /// ```
  fn box_it_clone<'a>(
    self,
  ) -> Self::With<Self::BoxedCoreObservableClone<'a, Self::Item<'a>, Self::Err>>
  where
    Self::Inner:
      IntoBoxedCoreObservable<Self::BoxedCoreObservableClone<'a, Self::Item<'a>, Self::Err>>,
  {
    self.transform(|inner| inner.into_boxed())
  }

  /// Type-erases the observable into a boxed mutable reference observable.
  ///
  /// This enables type erasure for observables that emit mutable references,
  /// useful for mut_ref broadcasting patterns.
  fn box_it_mut_ref<'a, Item>(
    self,
  ) -> Self::With<Self::BoxedCoreObservableMutRef<'a, Item, Self::Err>>
  where
    Self: 'a,
    Self::Item<'a>: std::ops::DerefMut<Target = Item> + 'a,
    Self::Inner: IntoBoxedCoreObservable<Self::BoxedCoreObservableMutRef<'a, Item, Self::Err>>,
  {
    self.transform(|inner| inner.into_boxed())
  }

  /// Type-erases the observable into a *cloneable* boxed mutable reference
  /// observable.
  ///
  /// This is only available when the underlying observable pipeline is
  /// `Clone` (enforced via trait bounds on the boxing target).
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Choose `Item = i32` so the observable emits `&mut i32`.
  /// let subject = Local::subject_mut_ref::<i32, Infallible>();
  /// let boxed = subject.clone().box_it_mut_ref_clone();
  /// let boxed2 = boxed.clone();
  ///
  /// boxed.subscribe(|_v: &mut i32| {});
  /// boxed2.subscribe(|_v: &mut i32| {});
  /// ```
  fn box_it_mut_ref_clone<'a, Item>(
    self,
  ) -> Self::With<Self::BoxedCoreObservableMutRefClone<'a, Item, Self::Err>>
  where
    Self: 'a,
    Self::Item<'a>: std::ops::DerefMut<Target = Item> + 'a,
    Self::Inner: IntoBoxedCoreObservable<Self::BoxedCoreObservableMutRefClone<'a, Item, Self::Err>>,
  {
    self.transform(|inner| inner.into_boxed())
  }

  /// Convert this observable into a Future that resolves with the last emitted
  /// value.
  ///
  /// This method subscribes to the observable and returns a future that
  /// resolves when the observable completes. It supports both synchronous and
  /// asynchronous observables.
  ///
  /// # Returns
  ///
  /// - `Ok(Ok(value))` - Observable emitted exactly one value
  /// - `Ok(Err(error))` - Observable emitted an error
  /// - `Err(IntoFutureError::Empty)` - Observable completed without emitting
  ///   any values
  /// - `Err(IntoFutureError::MultipleValues)` - Observable emitted more than
  ///   one value
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// #[cfg(not(target_arch = "wasm32"))]
  /// #[tokio::main]
  /// async fn main() {
  ///   // Single value
  ///   let fut = Local::of(42).into_future();
  ///   let value = fut.await.unwrap().ok().unwrap();
  ///   assert_eq!(value, 42);
  ///
  ///   // Empty observable
  ///   let fut = Local::from_iter(std::iter::empty::<i32>()).into_future();
  ///   assert!(matches!(fut.await, Err(IntoFutureError::Empty)));
  ///
  ///   // Multiple values
  ///   let fut = Local::from_iter([1, 2, 3]).into_future();
  ///   assert!(matches!(fut.await, Err(IntoFutureError::MultipleValues)));
  /// }
  /// #[cfg(target_arch = "wasm32")]
  /// fn main() {}
  /// ```
  fn into_future<'a>(self) -> ObservableFuture<Self::Item<'a>, Self::Err>
  where
    Self::Inner: SupportsIntoFuture<'a, Self>,
  {
    <Self::Inner as SupportsIntoFuture<'a, Self>>::into_future(self)
  }

  /// Convert this observable into a `futures_core::stream::Stream`.
  ///
  /// This method transforms the observable into an asynchronous stream,
  /// allowing you to iterate over the emitted values using standard async
  /// patterns like `while let` or stream combinators.
  ///
  /// The stream yields items of type `Result<Self::Item, Self::Err>`.
  ///
  /// # Requirements
  ///
  /// * The observable items must be `'static` (owned values) to safely cross
  ///   async boundaries.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use futures::stream::StreamExt;
  /// use rxrust::prelude::*;
  ///
  /// async fn test_stream() {
  ///   let mut stream = Local::of(42).into_stream();
  ///
  ///   while let Some(result) = stream.next().await {
  ///     match result {
  ///       Ok(value) => println!("Got value: {}", value),
  ///       Err(e) => println!("Error: {:?}", e),
  ///     }
  ///   }
  /// }
  /// ```
  fn into_stream<'a>(self) -> <Self::Inner as SupportsIntoStream<'a, Self>>::Stream
  where
    Self::Inner: SupportsIntoStream<'a, Self>,
  {
    <Self::Inner as SupportsIntoStream<'a, Self>>::into_stream(self)
  }

  /// Apply an accumulator function and emit only the final result
  ///
  /// This operator applies an accumulator function over the source Observable
  /// and returns only the final accumulated value when the source completes.
  /// If the source is empty, nothing is emitted.
  ///
  /// This is equivalent to `scan(Default::default(), f).last()`.
  ///
  /// # Arguments
  ///
  /// * `f` - A function that takes the current accumulator and a value, and
  ///   returns the new accumulator
  ///
  /// # Type Parameters
  ///
  /// * `Output` - The type of the accumulator value (must implement `Default`
  ///   and `Clone`)
  /// * `F` - The accumulator function type
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut result = 0;
  /// Local::from_iter([1, 2, 3, 4, 5])
  ///   .reduce(|acc: i32, v| acc + v)
  ///   .subscribe(|v| result = v);
  /// assert_eq!(result, 15);
  ///
  /// // On empty observable, nothing is emitted
  /// let mut emitted = false;
  /// Local::from_iter(std::iter::empty::<i32>())
  ///   .reduce(|acc: i32, v| acc + v)
  ///   .subscribe(|_| emitted = true);
  /// assert!(!emitted);
  /// ```
  #[allow(clippy::type_complexity)]
  fn reduce<'a, F>(self, f: F) -> Self::With<Reduce<Self::Inner, ReduceFn<F>, Self::Item<'a>>>
  where
    F: FnMut(Self::Item<'a>, Self::Item<'a>) -> Self::Item<'a>,
  {
    self.transform(|source| Reduce { source, strategy: ReduceFn(f), initial: None })
  }

  /// Apply an accumulator function with an initial value and emit the final
  /// result
  ///
  /// This operator applies an accumulator function over the source Observable,
  /// starting with an initial value, and returns the final accumulated value
  /// when the source completes. If the source is empty, the initial value is
  /// emitted.
  ///
  /// This is equivalent to `scan(initial, f).last().default_if_empty(initial)`.
  ///
  /// # Arguments
  ///
  /// * `initial` - The initial accumulator value
  /// * `f` - A function that takes the current accumulator and a value, and
  ///   returns the new accumulator
  ///
  /// # Type Parameters
  ///
  /// * `Output` - The type of the accumulator value
  /// * `F` - The accumulator function type
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut result = 0;
  /// Local::from_iter([1, 1, 1, 1, 1])
  ///   .reduce_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| result = v);
  /// assert_eq!(result, 105);
  ///
  /// // On empty observable, the initial value is emitted
  /// let mut result = 0;
  /// Local::from_iter(std::iter::empty::<i32>())
  ///   .reduce_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| result = v);
  /// assert_eq!(result, 100);
  /// ```
  fn reduce_initial<Output, F>(
    self, initial: Output, f: F,
  ) -> Self::With<Reduce<Self::Inner, ReduceInitialFn<F>, Output>>
  where
    F: for<'a> FnMut(Output, Self::Item<'a>) -> Output,
  {
    self.transform(|source| Reduce { source, strategy: ReduceInitialFn(f), initial: Some(initial) })
  }

  /// Emit the maximum item emitted by the source observable.
  ///
  /// **Empty source:** emits nothing.
  ///
  /// **Note:** This operator compares and returns owned items
  /// (`Item<'static>`).
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = None;
  /// Local::from_iter([3, 1, 4])
  ///   .max()
  ///   .subscribe(|v| out = Some(v));
  /// assert_eq!(out, Some(4));
  /// ```
  #[allow(clippy::type_complexity)]
  fn max<'a>(
    self,
  ) -> Self::With<
    Reduce<
      Self::Inner,
      ReduceFn<fn(Self::Item<'a>, Self::Item<'a>) -> Self::Item<'a>>,
      Self::Item<'a>,
    >,
  >
  where
    Self::Item<'a>: PartialOrd,
  {
    self.reduce(|acc, v| if v > acc { v } else { acc })
  }

  /// Emit the minimum item emitted by the source observable.
  ///
  /// **Empty source:** emits nothing.
  ///
  /// **Note:** This operator compares and returns owned items
  /// (`Item<'static>`).
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = None;
  /// Local::from_iter([3, 1, 4])
  ///   .min()
  ///   .subscribe(|v| out = Some(v));
  /// assert_eq!(out, Some(1));
  /// ```
  #[allow(clippy::type_complexity)]
  fn min<'a>(
    self,
  ) -> Self::With<
    Reduce<
      Self::Inner,
      ReduceFn<fn(Self::Item<'a>, Self::Item<'a>) -> Self::Item<'a>>,
      Self::Item<'a>,
    >,
  >
  where
    Self::Item<'a>: PartialOrd,
  {
    self.reduce(|acc, v| if v < acc { v } else { acc })
  }

  /// Emit the sum of items emitted by the source observable.
  ///
  /// **Empty source:** emits the additive identity (via `Default`).
  ///
  /// # Type requirements
  ///
  /// Items must implement `Default` and `Add<Output = Self>`.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = 0;
  /// Local::from_iter([1, 2, 3])
  ///   .sum()
  ///   .subscribe(|v| out = v);
  /// assert_eq!(out, 6);
  /// ```
  #[allow(clippy::type_complexity)]
  fn sum<'a>(
    self,
  ) -> Self::With<
    Reduce<
      Self::Inner,
      ReduceInitialFn<fn(Self::Item<'a>, Self::Item<'a>) -> Self::Item<'a>>,
      Self::Item<'a>,
    >,
  >
  where
    Self::Item<'a>: Default + std::ops::Add<Output = Self::Item<'a>>,
  {
    self.transform(|source| Reduce {
      source,
      strategy: ReduceInitialFn((|acc, v| acc + v) as _),
      initial: Some(Default::default()),
    })
  }

  /// Count the number of items emitted by the source observable.
  ///
  /// **Empty source:** emits `0`.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = 0;
  /// Local::from_iter(['a', 'b', 'c'])
  ///   .count()
  ///   .subscribe(|v| out = v);
  /// assert_eq!(out, 3);
  /// ```
  #[allow(clippy::type_complexity)]
  fn count(
    self,
  ) -> Self::With<
    Reduce<Self::Inner, ReduceInitialFn<for<'a> fn(usize, Self::Item<'a>) -> usize>, usize>,
  > {
    self.transform(|source| Reduce {
      source,
      strategy: ReduceInitialFn(
        (|acc, _v| acc + 1usize) as for<'a> fn(usize, Self::Item<'a>) -> usize,
      ),
      initial: Some(0),
    })
  }

  /// Emit the average of items emitted by the source observable
  ///
  /// This operator calculates the average of all items emitted by the source
  /// observable and emits the result when the source completes.
  ///
  /// # Requirements
  ///
  /// * `Item` must implement [`Averageable`] (e.g., `f32`, `f64`).
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// // Average of floats
  /// let mut result_f = 0.0;
  /// Local::from_iter([1.0, 2.0, 3.0])
  ///   .average()
  ///   .subscribe(|v| result_f = v);
  /// assert_eq!(result_f, 2.0);
  ///
  /// // Average of integers
  /// let mut result_i = 0;
  /// Local::from_iter([1, 2, 3, 4, 5])
  ///   .average()
  ///   .subscribe(|v| result_i = v);
  /// assert_eq!(result_i, 3);
  ///
  /// // Empty observable
  /// let mut emitted_empty = false;
  /// Local::from_iter(std::iter::empty::<f64>())
  ///   .average()
  ///   .subscribe(|_| emitted_empty = true);
  /// assert!(!emitted_empty);
  /// ```
  fn average(self) -> Self::With<Average<Self::Inner>>
  where
    for<'a> Self::Item<'a>: Averageable,
  {
    self.transform(Average::new)
  }

  /// Map each item into an inner observable and merge the results.
  ///
  /// This is equivalent to `map(f).merge_all(usize::MAX)`.
  ///
  /// - Subscribes to inner observables eagerly (unlimited concurrency).
  /// - Does **not** guarantee ordering across inners.
  /// - Errors from either the outer or any inner observable terminate
  ///   downstream.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = Vec::new();
  /// Local::from_iter([1_i32, 2_i32])
  ///   .flat_map(|x| Local::from_iter([x, x + 10]))
  ///   .subscribe(|v| out.push(v));
  /// assert_eq!(out, vec![1, 11, 2, 12]);
  /// ```
  fn flat_map<F, Inner>(self, f: F) -> Self::With<MergeAll<Map<Self::Inner, F>>>
  where
    F: for<'a> FnMut(Self::Item<'a>) -> Inner,
    Inner: Context<Inner: ObservableType<Err = Self::Err>>,
  {
    self.transform(|source| MergeAll { source: Map { source, func: f }, concurrent: usize::MAX })
  }

  /// Map each item into an inner observable and concatenate the results.
  ///
  /// This is equivalent to `map(f).concat_all()`.
  ///
  /// - Subscribes to inner observables sequentially (concurrency = 1).
  /// - Preserves ordering between inners.
  /// - Errors from either the outer or any inner observable terminate
  ///   downstream.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = Vec::new();
  /// Local::from_iter([1_i32, 2_i32])
  ///   .concat_map(|x| Local::from_iter([x, x + 10]))
  ///   .subscribe(|v| out.push(v));
  /// assert_eq!(out, vec![1, 11, 2, 12]);
  /// ```
  fn concat_map<F, Inner>(self, f: F) -> Self::With<MergeAll<Map<Self::Inner, F>>>
  where
    F: for<'a> FnMut(Self::Item<'a>) -> Inner,
    Inner: Context<Inner: ObservableType<Err = Self::Err>>,
  {
    self.transform(|source| MergeAll { source: Map { source, func: f }, concurrent: 1 })
  }

  /// Map each item into an inner observable and switch to the latest one.
  ///
  /// This operator transforms each item emitted by the source observable into
  /// a new observable. It subscribes to each inner observable, but when a new
  /// inner observable arrives, it unsubscribes from the previous one.
  ///
  /// - Only emits values from the most recent inner observable.
  /// - Previous inner observables are unsubscribed when new ones arrive.
  /// - Errors from either the outer or the current inner observable terminate
  ///   downstream.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  ///
  /// let mut out = Vec::new();
  /// Local::from_iter([1_i32, 2_i32, 3_i32])
  ///   .switch_map(|x| {
  ///     // Each number creates an observable that emits that number twice
  ///     Local::from_iter([x, x])
  ///   })
  ///   .subscribe(|v| out.push(v));
  /// assert_eq!(out, vec![1, 1, 2, 2, 3, 3]);
  /// ```
  fn switch_map<F, Out>(self, f: F) -> Self::With<SwitchMap<Self::Inner, F>>
  where
    F: for<'a> FnMut(Self::Item<'a>) -> Out,
    Out: Context<Inner: ObservableType<Err = Self::Err> + 'static>,
  {
    self.transform(|source| SwitchMap { source, func: f })
  }
}

pub type ConnectableObservableCtx<'a, O> = <O as Context>::With<
  ConnectableObservable<
    <O as Context>::Inner,
    SubjectPtr<'a, O, <O as Observable>::Item<'a>, <O as Observable>::Err>,
  >,
>;
pub type ConnectableObservableCtxMutRef<'a, O, Item> = <O as Context>::With<
  ConnectableObservable<
    <O as Context>::Inner,
    SubjectPtrMutRef<'a, O, Item, <O as Observable>::Err>,
  >,
>;

/// Blanket implementation of Observable for any Context
///
/// This implementation uses ObservableType to infer Item and Err types
/// from the CoreObservable implementation.
impl<T> Observable for T
where
  T: Context,
  T::Inner: ObservableType,
{
  type Item<'a>
    = <T::Inner as ObservableType>::Item<'a>
  where
    T: 'a;
  type Err = <T::Inner as ObservableType>::Err;
}
