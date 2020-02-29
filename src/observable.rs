mod trivial;
pub use trivial::*;

mod from_iter;
pub use from_iter::{from_iter, repeat};

mod of;
pub use of::{of, of_fn, of_option, of_result};

pub(crate) mod from_future;
pub use from_future::{from_future, from_future_result};

pub(crate) mod interval;
pub use interval::{interval, interval_at};

pub(crate) mod connectable_observable;
pub use connectable_observable::{
  ConnectableObservable, LocalConnectableObservable,
  SharedConnectableObservable,
};

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
use crate::prelude::*;
pub use observable_comp::*;

use ops::{
  box_it::{BoxOp, IntoBox},
  delay::DelayOp,
  filter::FilterOp,
  first::FirstOrOp,
  last::LastOrOp,
  map::MapOp,
  merge::MergeOp,
  observe_on::ObserveOnOp,
  scan::ScanOp,
  skip::SkipOp,
  skip_last::SkipLastOp,
  subscribe_on::SubscribeOnOP,
  take::TakeOp,
  take_last::TakeLastOp,
  throttle_time::{ThrottleEdge, ThrottleTimeOp},
  zip::ZipOp,
  Accum, AverageOp, CountOp, MinMaxOp, ReduceOp, SumOp,
};
use std::marker::PhantomData;
use std::ops::{Add, Mul};
use std::time::{Duration, Instant};

pub trait Observable {
  type Item;
  type Err;

  /// emit only the first item emitted by an Observable
  #[inline]
  fn first(self) -> TakeOp<Self>
  where
    Self: Sized,
  {
    self.take(1)
  }

  /// emit only the first item emitted by an Observable
  #[inline]
  fn first_or(self, default: Self::Item) -> FirstOrOp<TakeOp<Self>, Self::Item>
  where
    Self: Sized,
  {
    FirstOrOp {
      source: self.first(),
      default,
    }
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
  fn last_or(self, default: Self::Item) -> LastOrOp<Self, Self::Item>
  where
    Self: Sized,
  {
    LastOrOp {
      source: self,
      default: Some(default),
      last: None,
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
  fn last(self) -> LastOrOp<Self, Self::Item>
  where
    Self: Sized,
  {
    LastOrOp {
      source: self,
      default: None,
      last: None,
    }
  }

  /// Creates a new stream which calls a closure on each element and uses
  /// its return as the value.
  #[inline]
  fn map<B, Item, F>(self, f: F) -> MapOp<Self, F>
  where
    Self: Sized,
    F: Fn(B) -> Item,
  {
    MapOp {
      source: self,
      func: f,
    }
  }

  /// combine two Observables into one by merging their emissions
  ///
  /// # Example
  ///
  /// ```
  /// # use rxrust::prelude::*;
  /// let numbers = Subject::new();
  /// // crate a even stream by filter
  /// let even = numbers.clone().filter(|v| *v % 2 == 0);
  /// // crate an odd stream by filter
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
    Self: Sized,
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
    Self: Sized,
    F: Fn(&Self::Item) -> bool,
  {
    FilterOp {
      source: self,
      filter,
    }
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
    Self: Sized,
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
  fn skip(self, count: u32) -> SkipOp<Self>
  where
    Self: Sized,
  {
    SkipOp {
      source: self,
      count,
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
  fn skip_last(self, count: usize) -> SkipLastOp<Self>
  where
    Self: Sized,
  {
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
  fn take(self, count: u32) -> TakeOp<Self>
  where
    Self: Sized,
  {
    TakeOp {
      source: self,
      count,
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
  fn take_last(self, count: usize) -> TakeLastOp<Self>
  where
    Self: Sized,
  {
    TakeLastOp {
      source: self,
      count,
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
    Self: Sized,
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
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
    Self: Sized,
    BinaryOp: Fn(OutputItem, Self::Item) -> OutputItem,
    OutputItem: Default,
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
  fn reduce_initial<OutputItem, BinaryOp>(
    self,
    initial: OutputItem,
    binary_op: BinaryOp,
  ) -> ReduceOp<Self, BinaryOp, OutputItem>
  where
    Self: Sized,
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
  ) -> LastOrOp<ScanOp<Self, BinaryOp, OutputItem>, OutputItem>
  where
    Self: Sized,
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
  fn max(self) -> MinMaxOp<Self, Self::Item>
  where
    Self: Sized,
    Self::Item: Copy + Send + PartialOrd<Self::Item>,
  {
    fn get_greater<Item>(i: Option<Item>, v: Item) -> Option<Item>
    where
      Item: Copy + PartialOrd<Item>,
    {
      i.map(|vv| if vv < v { v } else { vv }).or(Some(v))
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
  fn min(self) -> MinMaxOp<Self, Self::Item>
  where
    Self: Sized,
    Self::Item: Copy + Send + PartialOrd<Self::Item>,
  {
    fn get_lesser<Item>(i: Option<Item>, v: Item) -> Option<Item>
    where
      Item: Copy + PartialOrd<Item>,
    {
      i.map(|vv| if vv > v { v } else { vv }).or(Some(v))
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
    Self: Sized,
    Self::Item: Copy + Default + Add<Self::Item, Output = Self::Item>,
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
  fn count(self) -> CountOp<Self, Self::Item>
  where
    Self: Sized,
  {
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
  fn average(self) -> AverageOp<Self, Self::Item>
  where
    Self: Sized,
    Self::Item: Copy
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
      T: Default + Copy + Send + Mul<f64, Output = T>,
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
      T: Copy + Add<T, Output = T>,
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
  fn publish<Subject: Default>(self) -> ConnectableObservable<Self, Subject>
  where
    Self: Sized,
  {
    ConnectableObservable {
      source: self,
      subject: Subject::default(),
    }
  }

  /// Delays the emission of items from the source Observable by a given timeout
  /// or until a given `Instant`.
  #[inline]
  fn delay(self, dur: Duration) -> DelayOp<Self>
  where
    Self: Sized,
  {
    DelayOp {
      source: self,
      delay: dur,
    }
  }

  #[inline]
  fn delay_at(self, at: Instant) -> DelayOp<Self>
  where
    Self: Sized,
  {
    DelayOp {
      source: self,
      delay: at.elapsed(),
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
  /// use rxrust::scheduler::Schedulers;
  /// use std::thread;
  ///
  /// let a = observable::from_iter(1..5).subscribe_on(Schedulers::NewThread);
  /// let b = observable::from_iter(5..10);
  /// a.merge(b).to_shared().subscribe(|v|{
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
  fn subscribe_on<SD>(self, scheduler: SD) -> SubscribeOnOP<Self, SD>
  where
    Self: Sized,
  {
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
  fn observe_on<'a, SD>(self, scheduler: SD) -> ObserveOnOp<'a, Self, SD>
  where
    Self: Sized,
  {
    ObserveOnOp {
      source: self,
      scheduler,
      _p: PhantomData,
    }
  }

  /// Emits a value from the source Observable, then ignores subsequent source
  /// values for duration milliseconds, then repeats this process.
  ///
  /// #Example
  /// ```
  /// use rxrust::{ prelude::*, ops::throttle_time::ThrottleEdge };
  /// use std::time::Duration;
  ///
  /// observable::interval(Duration::from_millis(1))
  ///   .to_shared()
  ///   .throttle_time(Duration::from_millis(9), ThrottleEdge::Leading)
  ///   .to_shared()
  ///   .subscribe(move |v| println!("{}", v));
  /// ```
  #[inline]
  fn throttle_time(
    self,
    duration: Duration,
    edge: ThrottleEdge,
  ) -> ThrottleTimeOp<Self>
  where
    Self: Sized,
  {
    ThrottleTimeOp {
      source: self,
      duration,
      edge,
    }
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
  fn zip<U>(self, other: U) -> ZipOp<Self, U>
  where
    Self: Sized,
    U: Observable,
  {
    ZipOp { a: self, b: other }
  }
}

pub trait LocalObservable<'a>: Observable {
  type Unsub: SubscriptionLike + 'static;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub;
}

#[doc(hidden)]
pub(crate) macro observable_proxy_impl(
  $ty: ident
  , $host: ident
  $(, $lf: lifetime)?
  $(, $generics: ident) *
) {
  impl<$($lf, )? $host, $($generics ,)*> Observable
    for $ty<$($lf, )? $host, $($generics ,)*>
  where
    $host: Observable
  {
    type Item = $host::Item;
    type Err = $host::Err;
  }
}
