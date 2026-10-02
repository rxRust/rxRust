//! OnErrorResumeNext operator implementation
//!
//! Continues with another observable when the source errors or completes.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription},
};

/// OnErrorResumeNext operator: Continue with `next` after the source ends
///
/// When the source errors (the error is discarded) or completes, `next` is
/// subscribed and mirrored. The output error type is `next`'s.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::throw_err("boom".to_string())
///   .map(|_| 0)
///   .on_error_resume_next(Local::from_iter(vec![1, 2]))
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "onErrorResumeNext")]
#[derive(Clone)]
pub struct OnErrorResumeNext<S, N> {
  pub source: S,
  pub next: N,
}

impl<S, N> ObservableType for OnErrorResumeNext<S, N>
where
  S: ObservableType,
  N: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = N::Err;
}

/// Observer that switches to `next` on any terminal event
pub struct OnErrorResumeNextObserver<Ctx: Context, N> {
  observer: Option<Ctx>,
  next: Option<N>,
  serial: Ctx::RcMut<Option<Ctx::BoxedSubscription>>,
}

// Hand-written so resubscribing operators upstream can clone this observer.
impl<Ctx: Context + Clone, N: Clone> Clone for OnErrorResumeNextObserver<Ctx, N> {
  fn clone(&self) -> Self {
    Self { observer: self.observer.clone(), next: self.next.clone(), serial: self.serial.clone() }
  }
}

impl<Ctx: Context, N> OnErrorResumeNextObserver<Ctx, N> {
  fn resume(mut self)
  where
    N: CoreObservable<Ctx, Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>>,
  {
    if let (Some(observer), Some(next)) = (self.observer.take(), self.next.take()) {
      let unsub = next.subscribe(observer);
      *self.serial.rc_deref_mut() = Some(unsub.into_boxed());
    }
  }
}

impl<Ctx, N, Item, SrcErr, OutErr> Observer<Item, SrcErr> for OnErrorResumeNextObserver<Ctx, N>
where
  Ctx: Context + Observer<Item, OutErr>,
  N: ObservableType<Err = OutErr>
    + CoreObservable<Ctx, Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>>,
{
  fn next(&mut self, value: Item) {
    if let Some(observer) = self.observer.as_mut() {
      observer.next(value);
    }
  }

  fn error(self, _err: SrcErr) { self.resume(); }

  fn complete(self) { self.resume(); }

  fn is_closed(&self) -> bool {
    self
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

impl<S, N, C> CoreObservable<C> for OnErrorResumeNext<S, N>
where
  C: Context,
  S: CoreObservable<C::With<OnErrorResumeNextObserver<C, N>>>,
  S::Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
  N: ObservableType,
  C::RcMut<Option<C::BoxedSubscription>>: Subscription,
{
  type Unsub = C::RcMut<Option<C::BoxedSubscription>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let OnErrorResumeNext { source, next } = self;
    let serial: C::RcMut<Option<C::BoxedSubscription>> = C::RcMut::from(None);
    let observer = OnErrorResumeNextObserver {
      observer: Some(context),
      next: Some(next),
      serial: serial.clone(),
    };
    let source_unsub = source.subscribe(C::lift(observer));
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
  fn test_resume_after_error() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    let mut source = Local::subject::<i32, String>();
    source
      .clone()
      .on_error_resume_next(Local::from_iter(vec![10, 11]))
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    source.next(1);
    source.error("boom".to_string());

    assert_eq!(*result.borrow(), vec![1, 10, 11]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_resume_after_completion() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter(vec![1])
      .on_error_resume_next(Local::from_iter(vec![2]))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_next_error_propagates() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();

    Local::from_iter(vec![1])
      .on_error_resume_next(Local::throw_err("later".to_string()).map(|_| 0))
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(error.borrow().as_deref(), Some("later"));
  }

  #[rxrust_macro::test]
  fn test_unsubscribe_cancels_next() {
    let source = Local::subject::<i32, String>();
    let next = Local::subject::<i32, Infallible>();

    let sub = source
      .clone()
      .on_error_resume_next(next.clone())
      .subscribe(|_| {});
    source.clone().complete();
    assert_eq!(next.inner.subscriber_count(), 1);
    sub.unsubscribe();
    assert_eq!(next.inner.subscriber_count(), 0);
  }
}
