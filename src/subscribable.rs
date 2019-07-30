use crate::prelude::*;

pub enum RxValue<'a, T, E> {
  Next(&'a T),
  Err(&'a E),
  Complete,
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

pub trait ImplSubscribable: Sized {
  /// The type of the elements being emitted.
  type Item;
  // The type of the error may propagating.
  type Err;

  fn subscribe_return_state(
    self,
    subscribe: impl RxFn(RxValue<'_, Self::Item, Self::Err>) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync>;
}

pub trait Subscribable: ImplSubscribable {
  /// Invokes an execution of an Observable and registers Observer handlers for
  /// notifications it will emit.
  ///
  /// * `error`: A handler for a terminal event resulting from an error.
  /// * `complete`: A handler for a terminal event resulting from successful
  /// completion.
  ///
  fn subscribe_err_complete(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    error: impl Fn(&Self::Err) + Send + Sync + 'static,
    complete: impl Fn() + Send + Sync + 'static,
  ) -> Box<dyn Subscription> {
    self.subscribe_return_state(RxFnWrapper::new(move |v: RxValue<'_, _, _>| {
      match v {
        RxValue::Next(v) => next(v),
        RxValue::Err(e) => error(e),
        RxValue::Complete => complete(),
      };

      RxReturn::Continue
    }))
  }

  fn subscribe_err(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    error: impl Fn(&Self::Err) + Send + Sync + 'static,
  ) -> Box<dyn Subscription> {
    self.subscribe_return_state(RxFnWrapper::new(move |v: RxValue<'_, _, _>| {
      match v {
        RxValue::Next(v) => next(v),
        RxValue::Err(e) => error(e),
        _ => {}
      };

      RxReturn::Continue
    }))
  }

  fn subscribe_complete(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    complete: impl Fn() + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self.subscribe_return_state(RxFnWrapper::new(move |v: RxValue<'_, _, _>| {
      match v {
        RxValue::Next(v) => next(v),
        RxValue::Complete => complete(),
        _ => {}
      };

      RxReturn::Continue
    }))
  }

  fn subscribe(self, next: impl Fn(&Self::Item) + Send + Sync + 'static) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self.subscribe_return_state(RxFnWrapper::new(move |v: RxValue<'_, _, _>| {
      if let RxValue::Next(v) = v {
        next(v);
      }

      RxReturn::Continue
    }))
  }
}

impl<'a, S: ImplSubscribable> Subscribable for S {}
