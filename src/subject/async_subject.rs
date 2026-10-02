//! AsyncSubject: emits only the last value, and only on completion.

use super::{replay_subject::Terminal, subject_core::Subject};
use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Shared state of an [`AsyncSubject`].
pub struct AsyncState<Item, Err> {
  last: Option<Item>,
  terminal: Option<Terminal<Err>>,
}

impl<Item, Err> Default for AsyncState<Item, Err> {
  fn default() -> Self { Self { last: None, terminal: None } }
}

/// A Subject that stores the last value and emits it to every subscriber only
/// when it completes. Subscribers arriving after completion receive the value
/// and completion; an error discards the value.
///
/// # Examples
///
/// ```rust
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let mut subject = Local::async_subject::<i32, std::convert::Infallible>();
/// let seen = Rc::new(RefCell::new(Vec::new()));
/// let sink = seen.clone();
/// subject
///   .clone()
///   .subscribe(move |v| sink.borrow_mut().push(v));
/// subject.next(1);
/// subject.next(2);
/// assert!(seen.borrow().is_empty());
/// subject.complete();
/// assert_eq!(*seen.borrow(), vec![2]);
/// ```
pub struct AsyncSubject<P, V> {
  /// The underlying subject that manages live subscribers
  pub subject: Subject<P>,
  state: V,
}

impl<P: Clone, V: Clone> Clone for AsyncSubject<P, V> {
  fn clone(&self) -> Self { Self { subject: self.subject.clone(), state: self.state.clone() } }
}

impl<P, V, Item, Err> Default for AsyncSubject<P, V>
where
  Subject<P>: Default,
  V: RcDerefMut<Target = AsyncState<Item, Err>> + From<AsyncState<Item, Err>>,
{
  fn default() -> Self {
    Self { subject: Subject::default(), state: V::from(AsyncState::default()) }
  }
}

impl<P, V, Item, Err> AsyncSubject<P, V>
where
  V: RcDeref<Target = AsyncState<Item, Err>>,
{
  /// Whether a terminal event has been recorded.
  pub fn is_terminated(&self) -> bool { self.state.rc_deref().terminal.is_some() }
}

impl<Item, Err, P, V> Observer<Item, Err> for AsyncSubject<P, V>
where
  Item: Clone,
  Err: Clone,
  V: RcDerefMut<Target = AsyncState<Item, Err>>,
  Subject<P>: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    if state.terminal.is_none() {
      state.last = Some(value);
    }
  }

  fn error(self, err: Err) {
    {
      let mut state = self.state.rc_deref_mut();
      if state.terminal.is_some() {
        return;
      }
      state.last = None;
      state.terminal = Some(Terminal::Error(err.clone()));
    }
    self.subject.error(err);
  }

  fn complete(self) {
    let last = {
      let mut state = self.state.rc_deref_mut();
      if state.terminal.is_some() {
        return;
      }
      state.terminal = Some(Terminal::Complete);
      state.last.clone()
    };
    let mut subject = self.subject;
    if let Some(value) = last {
      subject.next(value);
    }
    subject.complete();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().terminal.is_some() }
}

impl<P, V> ObservableType for AsyncSubject<P, V>
where
  Subject<P>: ObservableType,
{
  type Item<'a>
    = <Subject<P> as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <Subject<P> as ObservableType>::Err;
}

impl<Item, Err, C, P, V> CoreObservable<C> for AsyncSubject<P, V>
where
  C: Context + Observer<Item, Err>,
  Subject<P>: CoreObservable<C, Err = Err>,
  V: RcDerefMut<Target = AsyncState<Item, Err>>,
  Item: Clone,
  Err: Clone,
{
  type Unsub = Option<<Subject<P> as CoreObservable<C>>::Unsub>;

  fn subscribe(self, mut observer: C) -> Self::Unsub {
    let (last, terminal) = {
      let state = self.state.rc_deref();
      (state.last.clone(), state.terminal.clone())
    };
    match terminal {
      Some(Terminal::Error(err)) => {
        observer.error(err);
        None
      }
      Some(Terminal::Complete) => {
        if let Some(value) = last {
          observer.next(value);
        }
        observer.complete();
        None
      }
      None => Some(self.subject.subscribe(observer)),
    }
  }
}

/// The `AsyncSubject` type that `async_subject` builds for an observable `O`.
pub type AsyncSubjectOf<'a, O> = AsyncSubject<
  super::SubjectPtr<
    'a,
    O,
    <O as crate::observable::Observable>::Item<'a>,
    <O as crate::observable::Observable>::Err,
  >,
  <O as Context>::RcMut<
    AsyncState<
      <O as crate::observable::Observable>::Item<'a>,
      <O as crate::observable::Observable>::Err,
    >,
  >,
>;

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_async_subject_emits_last_on_complete() {
    let mut subject = Local::async_subject::<i32, Infallible>();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let seen_c = seen.clone();
    let completed_c = completed.clone();

    subject
      .clone()
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| seen_c.borrow_mut().push(v));

    subject.next(1);
    subject.next(2);
    assert!(seen.borrow().is_empty());

    subject.complete();
    assert_eq!(*seen.borrow(), vec![2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_async_subject_late_subscriber_after_complete() {
    let mut subject = Local::async_subject::<i32, Infallible>();
    subject.next(9);
    subject.clone().complete();

    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_c = seen.clone();
    subject
      .clone()
      .subscribe(move |v| seen_c.borrow_mut().push(v));

    assert_eq!(*seen.borrow(), vec![9]);
  }

  #[rxrust_macro::test]
  fn test_async_subject_empty_complete() {
    let subject = Local::async_subject::<i32, Infallible>();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let seen_c = seen.clone();
    let completed_c = completed.clone();

    subject
      .clone()
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| seen_c.borrow_mut().push(v));
    subject.complete();

    assert!(seen.borrow().is_empty());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_async_subject_error_discards_value() {
    let mut subject = Local::async_subject::<i32, String>();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(None));
    let seen_c = seen.clone();
    let error_c = error.clone();

    subject
      .clone()
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(move |v| seen_c.borrow_mut().push(v));
    subject.next(1);
    subject.error("boom".to_string());

    assert!(seen.borrow().is_empty());
    assert_eq!(error.borrow().as_deref(), Some("boom"));
  }
}
