use crate::prelude::*;

mod static_subscribe;
pub use static_subscribe::Subscribable;

mod subscribe_by_fn_ptr;
pub use subscribe_by_fn_ptr::SubscribableByFnPtr;
mod subscribe_by_box;
pub use subscribe_by_box::SubscribableByBox;

pub enum RxValue<T, E> {
  Next(T),
  Err(E),
  Complete,
}

impl<T, E> RxValue<T, E> {
  pub fn as_ref(&self) -> RxValue<&T, &E> {
    match self {
      RxValue::Next(n) => RxValue::Next(&n),
      RxValue::Err(e) => RxValue::Err(&e),
      RxValue::Complete => RxValue::Complete,
    }
  }

  pub fn as_mut(&mut self) -> RxValue<&T, &E> {
    match self {
      RxValue::Next(ref mut n) => RxValue::Next(n),
      RxValue::Err(ref mut e) => RxValue::Err(e),
      RxValue::Complete => RxValue::Complete,
    }
  }
}

impl<T, E> RxValue<&T, &E>
where
  T: Clone,
  E: Clone,
{
  pub fn to_owned(&self) -> RxValue<T, E> {
    match self {
      RxValue::Next(n) => RxValue::Next((*n).clone()),
      RxValue::Err(e) => RxValue::Err((*e).clone()),
      RxValue::Complete => RxValue::Complete,
    }
  }
}

/// The Observer's return state, can intervention the source stream's data
/// consume mechanism. `Continue` will do nothing, just let the source stream
/// consume data in its way, `Err` means should early termination because an
/// runtime error occur. `Complete` tell upstream I don't want to consume data
/// more, and let's complete now.
#[derive(PartialEq)]
pub enum RxReturn<E> {
  Continue,
  Err(E),
  Complete,
}

pub trait RawSubscribable {
  /// The type of the elements being emitted.
  type Item;
  // The type of the error may propagating.
  type Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync>;
}
