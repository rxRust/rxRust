//! Take operator implementation
//!
//! This module contains the Take operator, which emits only the first `count`
//! values emitted by the source Observable, then completes.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, single_assignment::State},
};

/// Take operator: Emits only the first `count` values from the source
/// observable
///
/// This operator emits only the first `count` values emitted by the source
/// observable, then completes. If the source completes before emitting `count`
/// values, `take` will also complete early.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).take(3);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3]);
/// ```
#[derive(Clone)]
pub struct Take<S> {
  pub source: S,
  pub count: usize,
}

impl<S: ObservableType> ObservableType for Take<S> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// TakeObserver wrapper for limiting the number of emitted values
///
/// This observer wraps another observer and only allows the first `count`
/// values to be emitted. After `count` values have been emitted, it calls
/// complete and prevents further values from being emitted.
pub struct TakeObserver<O, U> {
  upstream: U,
  observer: Option<O>,
  remaining: usize,
}

impl<O, U, Item, Err> Observer<Item, Err> for TakeObserver<O, U>
where
  O: Observer<Item, Err>,
  U: Subscription + Clone,
{
  fn next(&mut self, v: Item) {
    let should_complete = if self.remaining > 0
      && let Some(observer) = self.observer.as_mut()
    {
      observer.next(v);
      self.remaining -= 1;
      self.remaining == 0
    } else {
      false
    };

    if should_complete && let Some(observer) = self.observer.take() {
      observer.complete();
      self.upstream.clone().unsubscribe();
    }
  }

  fn error(self, e: Err) {
    if let Some(observer) = self.observer {
      observer.error(e);
    }
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

// The scheduler can cross threads even when selected from a Local context.
// A mutex-backed slot remains usable with borrowed and non-Send subscriptions.
type Handle<U> = SingleAssignment<crate::rc::MutArc<State<U>>>;

impl<'a, S: 'a, C, U> CoreObservable<C> for Take<S>
where
  C: Context,
  U: Subscription,
  // Infer U from an observer-independent witness before using it in the real
  // observer type. Without this bound Rust rejects U as unconstrained (E0207).
  S: CoreObservable<C::With<TakeObserver<C::Inner, ()>>, Unsub = U>,
  S: CoreObservable<C::With<TakeObserver<C::Inner, Handle<U>>>, Unsub = U>,
  C::Inner: Observer<S::Item<'a>, S::Err>,
{
  type Unsub = Handle<U>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Take { source, count } = self;
    let upstream = Handle::<U>::new();
    if count == 0 {
      context.into_inner().complete();
      upstream.clone().unsubscribe();
      return upstream;
    }
    let wrapped = context.transform(|observer| TakeObserver {
      observer: Some(observer),
      remaining: count,
      upstream: upstream.clone(),
    });
    upstream.set(source.subscribe(wrapped));
    upstream
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn cancels_synchronous_late_handle_without_static_or_clone_items() {
    use crate::subscription::ClosureSubscription;
    let cancelled = std::cell::Cell::new(0);
    struct NonClone;
    let mut seen = 0;
    Local::create(|emitter| {
      emitter.next(NonClone);
      emitter.next(NonClone);
      ClosureSubscription(|| cancelled.set(cancelled.get() + 1))
    })
    .take(1)
    .subscribe_with(crate::observer::FnMutObserver(|_| seen += 1));
    assert_eq!(seen, 1);
    assert_eq!(cancelled.get(), 1);
  }

  #[rxrust_macro::test]
  fn zero_completes_without_subscribing() {
    let mut completed = false;
    Local::create::<i32, std::convert::Infallible, _, ()>(|_| panic!("source started"))
      .take(0)
      .on_complete(|| completed = true)
      .subscribe_with(crate::observer::FnMutObserver(|_| {}));
    assert!(completed);
  }

  #[rxrust_macro::test]
  fn natural_termination_does_not_cancel() {
    use crate::subscription::ClosureSubscription;
    let cancelled = std::cell::Cell::new(0);
    Local::create(|emitter| {
      emitter.next(1);
      emitter.complete();
      ClosureSubscription(|| cancelled.set(cancelled.get() + 1))
    })
    .take(2)
    .subscribe_with(crate::observer::FnMutObserver(|_| {}));
    assert_eq!(cancelled.get(), 0);
  }

  #[rxrust_macro::test(local)]
  async fn first_and_first_or_cancel_active_source() {
    use crate::subscription::ClosureSubscription;
    let cancelled = Rc::new(std::cell::Cell::new(0));
    let c = cancelled.clone();
    let source = Local::create(move |emitter| {
      emitter.next(1);
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    source.clone().first().subscribe(|_| {});
    source.first_or(0).subscribe(|_| {});
    assert_eq!(cancelled.get(), 2);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_emits_specified_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take(3)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_completes_after_count() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take(2)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_with_zero_count() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take(0)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(|_| {
        // Should never be called
      });

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_with_count_greater_than_source() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .take(10)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take(3)
      .take(2)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_error_propagation() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let error = Rc::new(RefCell::new(String::new()));
    let result_clone = result.clone();
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .take(5)
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![]);
    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test(local)]
  async fn test_first() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .first()
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1]);
  }

  #[rxrust_macro::test(local)]
  async fn test_first_or() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .first_or(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1]);
  }

  #[rxrust_macro::test(local)]
  async fn test_first_or_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .first_or(0)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0]);
  }
}
