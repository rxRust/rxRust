use std::ops::Add;

use crate::{
  observable::{CoreObservable, ObservableType},
  ops::{
    filter_map::FilterMap,
    reduce::{Reduce, ReduceInitialFn},
  },
  subscription::Subscription,
};

/// A trait for types that can be averaged.
///
/// This trait extends `Add` to support accumulation.
/// It adds a `div_count` method to calculate the final average by dividing the
/// sum by the count of items.
pub trait Averageable: Add<Output = Self> + Sized {
  /// Divides the accumulated sum by the count of items to produce the average.
  fn div_count(self, count: usize) -> Self;
}

impl Averageable for f32 {
  fn div_count(self, count: usize) -> Self { self / (count as f32) }
}

impl Averageable for f64 {
  fn div_count(self, count: usize) -> Self { self / (count as f64) }
}

/// Implements `Averageable` for integer types.
///
/// This macro provides a blanket implementation of the `Averageable` trait
/// for various integer types, allowing them to be used with the `average`
/// operator.
///
/// The `div_count` method for integers performs integer division.
macro_rules! impl_averageable_int {
  ($($t:ty),*) => {
    $(
      impl Averageable for $t {
        fn div_count(self, count: usize) -> Self {
          self / (count as $t)
        }
      }
    )*
  };
}

impl_averageable_int!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

/// Average operator (type-level wrapper to avoid huge nested types)
///
/// The actual implementation is built as a small pipeline (reduce + filter_map)
/// and delegated to in `subscribe`.
#[derive(Clone)]
pub struct Average<Source> {
  pub(crate) source: Source,
}

impl<Source> Average<Source> {
  #[inline]
  pub fn new(source: Source) -> Self { Self { source } }
}

impl<Source> ObservableType for Average<Source>
where
  Source: ObservableType,
{
  type Item<'a>
    = Source::Item<'a>
  where
    Self: 'a;

  type Err = Source::Err;
}

type AvgAcc<Item> = (Option<Item>, usize);

type AveragePipeline<Source, Item> = FilterMap<
  Reduce<Source, ReduceInitialFn<fn(AvgAcc<Item>, Item) -> AvgAcc<Item>>, AvgAcc<Item>>,
  fn(AvgAcc<Item>) -> Option<Item>,
>;

#[inline]
fn build_pipeline<'a, Source>(source: Source) -> AveragePipeline<Source, Source::Item<'a>>
where
  Source: ObservableType,
  Source::Item<'a>: Averageable,
{
  type Item<'a, S> = <S as ObservableType>::Item<'a>;

  type ReducerFn<'a, S> = fn(AvgAcc<Item<'a, S>>, Item<'a, S>) -> AvgAcc<Item<'a, S>>;

  let reducer: ReducerFn<'a, Source> = (|acc, value| match acc {
    (Some(sum), count) => (Some(sum + value), count + 1usize),
    (None, count) => (Some(value), count + 1usize),
  }) as _;

  let finish: fn(AvgAcc<Item<'a, Source>>) -> Option<Item<'a, Source>> =
    (|(sum, count): AvgAcc<Item<'a, Source>>| sum.map(|s| s.div_count(count))) as _;

  FilterMap {
    source: Reduce { source, strategy: ReduceInitialFn(reducer), initial: Some((None, 0usize)) },
    func: finish,
  }
}

impl<Source, C, Unsub> CoreObservable<C> for Average<Source>
where
  Source: ObservableType,
  for<'a> Source::Item<'a>: Averageable,
  for<'a> AveragePipeline<Source, Source::Item<'a>>: CoreObservable<C, Unsub = Unsub>,
  Unsub: Subscription,
{
  type Unsub = Unsub;

  #[inline]
  fn subscribe(self, observer: C) -> Self::Unsub {
    build_pipeline::<Source>(self.source).subscribe(observer)
  }
}
