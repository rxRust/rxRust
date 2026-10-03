//! TakeWhile operator implementation
//!
//! This module contains the TakeWhile operator, which emits values from the
//! source observable as long as a predicate returns true. When the predicate
//! returns false, it completes the stream.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, single_assignment::State},
};

/// TakeWhile operator: Emits values while a predicate returns true
///
/// This operator emits values from the source observable as long as the
/// predicate function returns true. When the predicate returns false,
/// it completes the stream and stops emitting values.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::from_iter([1, 2, 3, 4, 5]).take_while(|v| *v < 4);
/// let mut result = Vec::new();
/// observable.subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3]);
/// ```
#[derive(Clone)]
pub struct TakeWhile<S, P> {
  pub source: S,
  pub predicate: P,
  pub inclusive: bool,
}

// todo: 优化实现。
impl<S: ObservableType, P> ObservableType for TakeWhile<S, P> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

/// TakeWhileObserver wrapper for conditional value emission
///
/// This observer wraps another observer and emits values as long as
/// the predicate returns true. When predicate returns false, it
/// completes and stops further emissions.
pub struct TakeWhileObserver<O, P, U> {
  upstream: U,
  observer: Option<O>,
  predicate: P,
  inclusive: bool,
}

impl<O, P, U, Item, Err> Observer<Item, Err> for TakeWhileObserver<O, P, U>
where
  U: Subscription + Clone,
  O: Observer<Item, Err>,
  P: FnMut(&Item) -> bool,
{
  fn next(&mut self, v: Item) {
    if let Some(ref mut observer) = self.observer {
      if (self.predicate)(&v) {
        observer.next(v);
      } else {
        // Predicate returned false
        if self.inclusive {
          observer.next(v);
        }
        // Complete and stop further emissions
        if let Some(observer) = self.observer.take() {
          observer.complete();
          self.upstream.clone().unsubscribe();
        }
      }
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

impl<S, P, C, U> CoreObservable<C> for TakeWhile<S, P>
where
  C: Context,
  U: Subscription,
  S: CoreObservable<C::With<TakeWhileObserver<C::Inner, P, ()>>, Unsub = U>
    + CoreObservable<C::With<TakeWhileObserver<C::Inner, P, Handle<U>>>, Unsub = U>,
{
  type Unsub = Handle<U>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let upstream = Handle::<U>::new();
    let wrapped = context.transform(|observer| TakeWhileObserver {
      observer: Some(observer),
      predicate: self.predicate,
      inclusive: self.inclusive,
      upstream: upstream.clone(),
    });
    upstream.set(self.source.subscribe(wrapped));
    upstream
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn short_circuit_cancels_late_handle() {
    use crate::subscription::ClosureSubscription;
    let calls = Rc::new(std::cell::Cell::new(0));
    for inclusive in [false, true] {
      let c = calls.clone();
      Local::create(move |e| {
        e.next(1);
        ClosureSubscription(move || c.set(c.get() + 1))
      })
      .transform(|source| super::TakeWhile { source, predicate: |_: &i32| false, inclusive })
      .subscribe(|_| {});
    }
    assert_eq!(calls.get(), 2);
  }
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_take_while_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take_while(|v| *v < 4)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_while_inclusive() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .take_while_inclusive(|v| *v < 4)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_while_all_pass() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3])
      .take_while(|v| *v < 100)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_take_while_none_pass() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter([10, 20, 30])
      .take_while(|v| *v < 5)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_take_while_chaining() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
      .take_while(|v| *v < 8)
      .take_while(|v| *v < 5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn test_take_while_error_propagation() {
    let error = Rc::new(RefCell::new(String::new()));
    let error_clone = error.clone();

    Local::throw_err("test error".to_string())
      .take_while(|_: &()| true)
      .on_error(move |e| {
        *error_clone.borrow_mut() = e;
      })
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), "test error");
  }

  #[rxrust_macro::test]
  fn test_take_while_with_map() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3, 4, 5])
      .map(|v| v * 2)
      .take_while(|v| *v < 8)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // 1*2=2, 2*2=4, 3*2=6, 4*2=8 (stops here, >= 8)
    assert_eq!(*result.borrow(), vec![2, 4, 6]);
  }
}
