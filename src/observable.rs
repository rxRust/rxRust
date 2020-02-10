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
pub use connectable_observable::{Connect, ConnectableObservable};

mod base;
pub use base::*;

pub mod from_fn;
pub use from_fn::{create, ObservableFromFn};

mod observable_all;
pub use observable_all::*;
mod observable_err;
pub use observable_err::*;
mod observable_next;
pub use observable_next::*;
mod observable_comp;
pub use observable_comp::*;

use crate::prelude::*;
use std::sync::{Arc, Mutex};

pub trait IntoShared {
  type Shared: Sync + Send + 'static;
  fn to_shared(self) -> Self::Shared;
}

pub trait Observable<'a> {
  type Item;
  type Err;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> LocalSubscription;
}

impl<Item, Err> IntoShared for Box<dyn Observer<Item, Err> + Send + Sync>
where
  Item: 'static,
  Err: 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}
impl<S> IntoShared for Arc<Mutex<S>>
where
  S: Send + Sync + 'static,
{
  type Shared = Self;
  #[inline(always)]
  fn to_shared(self) -> Self::Shared { self }
}
