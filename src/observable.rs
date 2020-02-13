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
  LocalConnectableObservable, SharedConnectableObservable,
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

pub trait Observable<'a> {
  type Item;
  type Err;
  type Unsub: SubscriptionLike + 'static;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub;

  /// Returns a ConnectableObservable. A ConnectableObservable Observable
  /// resembles an ordinary Observable, except that it does not begin emitting
  /// items when it is subscribed to, but only when the Connect operator is
  /// applied to it. In this way you can wait for all intended observers to
  /// subscribe to the Observable before the Observable begins emitting items.
  ///
  #[inline(always)]
  fn publish(
    self,
  ) -> LocalConnectableObservable<'a, Self, Self::Item, Self::Err>
  where
    Self: Sized,
  {
    LocalConnectableObservable::local(self)
  }
}
