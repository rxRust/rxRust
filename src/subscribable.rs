use crate::prelude::*;

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

pub trait RawSubscribable: Sized {
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

pub trait Subscribable: RawSubscribable {
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
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
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
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
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
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
      match v {
        RxValue::Next(v) => next(v),
        RxValue::Complete => complete(),
        _ => {}
      };

      RxReturn::Continue
    }))
  }

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
      if let RxValue::Next(v) = v {
        next(v);
      }

      RxReturn::Continue
    }))
  }
}

impl<'a, S: RawSubscribable> Subscribable for S {}
