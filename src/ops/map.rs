//! Map operator implementation
//!
//! This module contains the Map operator, which transforms items emitted by the
//! source observable. It demonstrates the observer wrapping pattern and
//! transformation logic.

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Map operator: Transforms items emitted by the source observable
///
/// This operator demonstrates:
/// - Observer wrapping (MapObserver)
/// - Context unpacking with `transform`
/// - Generic transformation function
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let observable = Local::of(42).map(|x| x * 2);
/// let mut result = None;
/// observable.subscribe(|v| {
///   result = Some(v);
/// });
/// assert_eq!(result, Some(84));
/// ```
#[derive(Clone)]
pub struct Map<S, F> {
  pub source: S,
  pub func: F,
}

/// MapObserver wrapper for transforming values
///
/// This observer wraps another observer and applies a transformation function
/// to each value before passing it to the wrapped observer.
pub struct MapObserver<O, F> {
  observer: O,
  func: F,
}

#[cfg(not(feature = "nightly"))]
impl<O, F, Item, Out, Err> Observer<Item, Err> for MapObserver<O, F>
where
  O: Observer<Out, Err>,
  F: FnMut(Item) -> Out,
{
  fn next(&mut self, v: Item) { self.observer.next((self.func)(v)); }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

#[cfg(feature = "nightly")]
impl<O, F, Item, Err> Observer<Item, Err> for MapObserver<O, F>
where
  F: FnMut<(Item,)>,
  O: Observer<<F as FnOnce<(Item,)>>::Output, Err>,
{
  fn next(&mut self, v: Item) { self.observer.next((self.func)(v)); }

  fn error(self, e: Err) { self.observer.error(e); }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

#[cfg(not(feature = "nightly"))]
impl<S, F, Out> ObservableType for Map<S, F>
where
  S: ObservableType,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
{
  type Item<'a>
    = Out
  where
    Self: 'a;
  type Err = S::Err;
}

#[cfg(feature = "nightly")]
impl<S, F> ObservableType for Map<S, F>
where
  S: ObservableType,
  F: for<'a> FnMut<(S::Item<'a>,)>,
{
  type Item<'a>
    = <F as FnOnce<(S::Item<'a>,)>>::Output
  where
    Self: 'a;
  type Err = S::Err;
}

#[cfg(not(feature = "nightly"))]
impl<S, F, C, Out> CoreObservable<C> for Map<S, F>
where
  C: Context,
  S: CoreObservable<C::With<MapObserver<C::Inner, F>>>,
  F: for<'a> FnMut(S::Item<'a>) -> Out,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Map { source, func } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| MapObserver { observer, func });
    source.subscribe(wrapped)
  }
}

#[cfg(feature = "nightly")]
impl<S, F, C> CoreObservable<C> for Map<S, F>
where
  C: Context,
  S: CoreObservable<C::With<MapObserver<C::Inner, F>>>,
  F: for<'a> FnMut<(S::Item<'a>,)>,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Map { source, func } = self;
    // Use transform to preserve scheduler automatically
    let wrapped = context.transform(|observer| MapObserver { observer, func });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_map_transforms_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::of(1).map(|x| x * 2).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_map_chaining() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::of(3)
      .map(|x| x + 1)
      .map(|x| x * 2)
      .subscribe(move |v| {
        *result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*result.borrow(), Some(8)); // (3 + 1) * 2 = 8
  }

  #[rxrust_macro::test(local)]
  async fn test_map_different_types() {
    let string_result = Rc::new(RefCell::new(None));
    let string_result_clone = string_result.clone();

    Local::of(42)
      .map(|x| format!("Number: {}", x))
      .subscribe(move |v| {
        *string_result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*string_result.borrow(), Some("Number: 42".to_string()));

    let bool_result = Rc::new(RefCell::new(None));
    let bool_result_clone = bool_result.clone();

    Local::of("hello world".to_string())
      .map(|s: String| s.len() > 5)
      .subscribe(move |v| {
        *bool_result_clone.borrow_mut() = Some(v);
      });

    assert_eq!(*bool_result.borrow(), Some(true)); // "hello world".len() = 11, so true
  }

  #[cfg(feature = "nightly")]
  #[rxrust_macro::test(local)]
  async fn test_map_borrowed_output() {
    use std::convert::Infallible;

    let subject = Local::subject_mut_ref::<i32, Infallible>();
    let mut producer = subject.clone();

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    fn as_ref(v: &mut i32) -> &i32 { v }

    subject.map(as_ref).subscribe(move |v: &i32| {
      result_clone.borrow_mut().push(*v);
    });

    let mut value = 10;
    producer.next(&mut value);

    assert_eq!(*result.borrow(), vec![10]);
  }
}
