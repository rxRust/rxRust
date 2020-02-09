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
pub mod take_last;
pub use take_last::TakeLast;
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
#[derive(Clone)]
pub struct SharedOp<T: Send + Sync + 'static>(pub(crate) T);

impl<O, U, OP> Observable<O, U> for SharedOp<OP>
where
  O: IntoShared,
  U: SubscriptionLike + IntoShared,
  U::Shared: SubscriptionLike,
  OP: Observable<O::Shared, U::Shared> + Send + Sync + 'static,
{
  type Unsub = OP::Unsub;
  fn actual_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    self.0.actual_subscribe(subscriber.to_shared())
  }
}

impl<OP: Send + Sync + 'static> Fork for SharedOp<OP>
where
  OP: Fork,
  OP::Output: Send + Sync + 'static,
{
  type Output = SharedOp<OP::Output>;
  fn fork(&self) -> Self::Output { SharedOp(self.0.fork()) }
}

impl<S: Send + Sync + 'static> IntoShared for SharedOp<S> {
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}
