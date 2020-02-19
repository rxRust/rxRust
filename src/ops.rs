pub mod last;
pub use last::Last;
pub mod reduce;
pub use reduce::Reduce;
pub mod minmax;
pub use minmax::MinMax;
pub mod sum;
pub use sum::Sum;
pub mod count;
pub use count::Count;
pub mod average;
pub use average::Average;
pub mod map;
pub use map::Map;
pub mod filter;
pub use filter::Filter;
pub mod scan;
pub use scan::Scan;
pub mod merge;
pub use merge::Merge;
pub mod take;
pub use take::Take;
pub mod skip;
pub use skip::Skip;
pub mod take_last;
pub use take_last::TakeLast;
pub mod skip_last;
pub use skip_last::SkipLast;
pub mod first;
pub use first::{First, FirstOr};
pub mod fork;
pub use fork::Fork;
pub mod subscribe_on;
pub use subscribe_on::SubscribeOn;
pub mod observe_on;
pub use observe_on::ObserveOn;
pub mod delay;
pub use delay::Delay;
pub mod throttle_time;
pub use throttle_time::{ThrottleEdge, ThrottleTime};
pub mod publish;
pub use publish::Publish;
pub mod filter_map;
pub mod ref_count;
pub use filter_map::FilterMap;

use crate::prelude::*;
pub struct SharedOp<T>(pub(crate) T);

impl<S, OP> RawSubscribable<S> for SharedOp<OP>
where
  OP: RawSubscribable<S::Shared>,
  S: IntoShared,
{
  type Unsub = OP::Unsub;
  fn raw_subscribe(self, subscriber: S) -> Self::Unsub {
    self.0.raw_subscribe(subscriber.to_shared())
  }
}

impl<OP> Fork for SharedOp<OP>
where
  OP: Fork,
{
  type Output = SharedOp<OP::Output>;
  fn fork(&self) -> Self::Output { SharedOp(self.0.fork()) }
}

impl<S> IntoShared for SharedOp<S>
where
  S: IntoShared + Send + Sync + 'static,
{
  type Shared = SharedOp<S::Shared>;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { SharedOp(self.0.to_shared()) }
}
