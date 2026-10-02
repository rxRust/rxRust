//! CatchError operator implementation
//!
//! Recovers from an error by switching to a fallback observable.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription},
};

/// CatchError operator: On error, subscribe to a fallback observable
///
/// The handler receives the error and returns the observable to continue
/// with. Its items must match the source's; its error type becomes the
/// output error type.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::throw_err("boom".to_string())
///   .map(|_| 0)
///   .catch_error(|e: String| Local::from_iter(vec![e.len() as i32]))
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![4]);
/// ```
#[doc(alias = "catchError")]
#[derive(Clone)]
pub struct CatchError<S, F> {
  pub source: S,
  pub handler: F,
}

impl<S, F, Out> ObservableType for CatchError<S, F>
where
  S: ObservableType,
  F: FnMut(S::Err) -> Out,
  Out: Context<Inner: ObservableType>,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = <Out::Inner as ObservableType>::Err;
}

/// Observer that swaps in the fallback on error
pub struct CatchErrorObserver<Ctx: Context, F> {
  observer: Option<Ctx>,
  handler: F,
  serial: Ctx::RcMut<Option<Ctx::BoxedSubscription>>,
}

// Hand-written so resubscribing operators upstream (`retry`, `repeat`) can
// clone this observer; a derive would demand `BoxedSubscription: Clone`.
impl<Ctx: Context + Clone, F: Clone> Clone for CatchErrorObserver<Ctx, F> {
  fn clone(&self) -> Self {
    Self {
      observer: self.observer.clone(),
      handler: self.handler.clone(),
      serial: self.serial.clone(),
    }
  }
}

impl<Ctx, F, Out, Item, SrcErr, OutErr> Observer<Item, SrcErr> for CatchErrorObserver<Ctx, F>
where
  Ctx: Context + Observer<Item, OutErr>,
  F: FnMut(SrcErr) -> Out,
  Out: Context<
    Inner: ObservableType<Err = OutErr>
             + CoreObservable<Ctx, Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>>,
  >,
{
  fn next(&mut self, value: Item) {
    if let Some(observer) = self.observer.as_mut() {
      observer.next(value);
    }
  }

  fn error(mut self, err: SrcErr) {
    let Some(observer) = self.observer.take() else { return };
    let fallback = (self.handler)(err).into_inner();
    let unsub = fallback.subscribe(observer);
    *self.serial.rc_deref_mut() = Some(unsub.into_boxed());
  }

  fn complete(self) {
    if let Some(observer) = self.observer {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool {
    self
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

impl<S, F, C, Out> CoreObservable<C> for CatchError<S, F>
where
  C: Context,
  F: FnMut(S::Err) -> Out,
  Out: Context<Inner: ObservableType>,
  S: CoreObservable<C::With<CatchErrorObserver<C, F>>>,
  S::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  C::RcMut<Option<C::BoxedSubscription>>: Subscription,
{
  type Unsub = C::RcMut<Option<C::BoxedSubscription>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let CatchError { source, handler } = self;
    let serial: C::RcMut<Option<C::BoxedSubscription>> = C::RcMut::from(None);
    let observer = CatchErrorObserver { observer: Some(context), handler, serial: serial.clone() };
    let source_unsub = source.subscribe(C::lift(observer));
    // The fallback may have replaced the slot synchronously; keep that one.
    let mut slot = serial.rc_deref_mut();
    if slot.is_none() {
      *slot = Some(source_unsub.into_boxed());
    }
    drop(slot);
    serial
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_catch_error_switches_to_fallback() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let mut source = Local::subject::<i32, String>();
    source
      .clone()
      .catch_error(|e: String| Local::from_iter(vec![e.len() as i32, 100]))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    source.next(1);
    source.error("boom".to_string());

    assert_eq!(*result.borrow(), vec![1, 4, 100]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_catch_error_transparent_without_error() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(vec![1, 2])
      .map_err(|_: Infallible| String::new())
      .catch_error(|_: String| Local::of(0))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_catch_error_fallback_error_propagates() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::throw_err("first".to_string())
      .map(|_| 0)
      .catch_error(|_: String| Local::throw_err(42u8).map(|_| 0))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), Some(42u8));
  }

  #[rxrust_macro::test]
  fn test_catch_error_unsubscribe_cancels_fallback() {
    let source = Local::subject::<i32, String>();
    let fallback = Local::subject::<i32, Infallible>();
    let fallback_c = fallback.clone();

    let sub = source
      .clone()
      .catch_error(move |_: String| fallback_c.clone())
      .subscribe(|_| {});

    source.error("boom".to_string());
    assert_eq!(fallback.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert_eq!(fallback.inner.subscriber_count(), 0);
  }
}
