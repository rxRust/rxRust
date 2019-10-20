pub mod map;
pub use map::Map;
pub mod filter;
pub use filter::Filter;
pub mod merge;
pub use merge::Merge;
pub mod take;
pub use take::Take;
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
// pub mod throttle_time;
// pub use throttle_time::ThrottleTime;

use crate::prelude::*;
pub struct SharedOp<T>(T);

impl<Item, Err, S, OP> RawSubscribable<Item, Err, S> for SharedOp<OP>
where
  OP: RawSubscribable<Item, Err, S::Shared>,
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
  S: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}
