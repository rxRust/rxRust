use crate::prelude::*;

pub enum OState<E> {
  Next,
  Complete,
  Err(E),
}

pub trait ImplSubscribable: Sized {
  /// The type of the elements being emitted.
  type Item;
  // The type of the error may propagating.
  type Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + Send + Sync + 'static,
    error: Option<impl Fn(&Self::Err) + Send + Sync + 'static>,
    complete: Option<impl Fn() + Send + Sync + 'static>,
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
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      Some(error),
      Some(complete),
    )
  }

  fn subscribe_err(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    error: impl Fn(&Self::Err) + Send + Sync + 'static,
  ) -> Box<dyn Subscription> {
    let complete: Option<fn()> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      Some(error),
      complete,
    )
  }

  fn subscribe_complete(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
    complete: impl Fn() + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    let err: Option<fn(&Self::Err)> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      err,
      Some(complete),
    )
  }

  fn subscribe(
    self,
    next: impl Fn(&Self::Item) + Send + Sync + 'static,
  ) -> Box<dyn Subscription>
  where
    Self::Err: 'static,
  {
    let complete: Option<fn()> = None;
    let err: Option<fn(&Self::Err)> = None;
    self.subscribe_return_state(
      move |v| {
        next(v);
        OState::Next
      },
      err,
      complete,
    )
  }
}

impl<'a, S: ImplSubscribable> Subscribable for S {}
