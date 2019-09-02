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

pub trait RawSubscribable {
  /// The type of the elements being emitted.
  type Item;
  // The type of the error may propagating.
  type Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
    + Send
    + Sync
    + 'static,
  ) -> Box<dyn Subscription + Send + Sync>;
}

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
  fn subscribe_all(
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
  ) -> Box<dyn Subscription>;

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>;

  /// Convert a Subscribable to Subject. This is different to [`Fork`]. `fork`
  /// only fork a new stream from the origin, it's a lazy operator, but
  /// `into_subject` will subscribe origin stream immediately and return an
  /// subject.
  fn into_subject(self) -> Subject<Self::Item, Self::Err>
  where
    Self::Item: 'static,
    Self::Err: 'static;
}

impl<S: RawSubscribable> Subscribable for S {
  type Item = S::Item;
  type Err = S::Err;
  fn subscribe_all(
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
    }))
  }

  fn subscribe_complete(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    complete: impl Fn() + Send + Sync + 'static,
  ) -> Box<dyn Subscription> {
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
      match v {
        RxValue::Next(v) => next(v),
        RxValue::Complete => complete(),
        _ => {}
      };
    }))
  }

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription> {
    self.raw_subscribe(RxFnWrapper::new(move |v: RxValue<&'_ _, &'_ _>| {
      if let RxValue::Next(v) = v {
        next(v);
      }
    }))
  }
  #[inline(always)]
  fn into_subject(self) -> Subject<Self::Item, Self::Err>
  where
    Self::Item: 'static,
    Self::Err: 'static,
  {
    Subject::from_subscribable(self)
  }
}
