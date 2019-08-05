use crate::prelude::*;

pub trait Subscribable {
  type Item;
  type Err;
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
  ) -> Box<dyn Subscription>;

  fn subscribe_err(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    error: impl Fn(&Self::Err) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>;

  fn subscribe_complete(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    complete: impl Fn() + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static;

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static;
}

impl<S: RawSubscribable> Subscribable for S {
  type Item = S::Item;
  type Err = S::Err;
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
