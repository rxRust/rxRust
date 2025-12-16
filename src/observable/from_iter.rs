//! FromIter operator implementation
//!
//! Creates an Observable from an Iterator that emits each item synchronously
//! when subscribed.

use std::convert::Infallible;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Creates an Observable from an Iterator that emits each item synchronously
/// when subscribed.
///
/// This operator converts an iterator into an observable that emits all items
/// from the iterator in order when subscribed.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// // Create an observable from a range
/// let obs = Local::from_iter(0..5);
/// obs.subscribe(|value| println!("Value: {}", value));
/// // Output: Value: 0, Value: 1, Value: 2, Value: 3, Value: 4
/// ```
pub fn from_iter<Iter>(iter: Iter) -> FromIter<Iter>
where
  Iter: IntoIterator,
{
  FromIter { iter }
}

/// Observable that emits all items from an iterator when subscribed.
#[derive(Clone)]
pub struct FromIter<Iter> {
  iter: Iter,
}

impl<Iter> ObservableType for FromIter<Iter>
where
  Iter: IntoIterator,
{
  type Item<'a>
    = Iter::Item
  where
    Self: 'a;
  type Err = Infallible;
}

impl<C, Iter> CoreObservable<C> for FromIter<Iter>
where
  C: Context,
  C::Inner: Observer<Iter::Item, Infallible>,
  Iter: IntoIterator,
{
  type Unsub = ();

  fn subscribe(self, context: C) -> Self::Unsub {
    let mut observer = context.into_inner();

    for item in self.iter {
      if observer.is_closed() {
        break;
      }
      observer.next(item);
    }

    if !observer.is_closed() {
      observer.complete();
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_from_iter_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter([1, 2, 3]).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test(local)]
  async fn test_from_iter_range() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    Local::from_iter(0..5).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(*result.borrow(), vec![0, 1, 2, 3, 4]);
  }

  #[rxrust_macro::test(local)]
  async fn test_from_iter_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(std::iter::empty::<i32>())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert!(result.borrow().is_empty());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_iter_vec() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(vec![0; 5])
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(result.borrow().len(), 5);
    assert_eq!(*result.borrow(), vec![0, 0, 0, 0, 0]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_iter_with_take() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(0..)
      .take(5)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0, 1, 2, 3, 4]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_from_iter_factory_method() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_clone = result.clone();
    let completed_clone = completed.clone();

    Local::from_iter(0..3)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![0, 1, 2]);
    assert!(*completed.borrow());
  }
}
