//! ElementAt operator implementation
//!
//! Emits the item at a zero-based index. Composed from `skip` and `take`,
//! with `default_if_empty` for the `_or` form.

use crate::ops::{default_if_empty::DefaultIfEmpty, skip::Skip, take::Take};

/// Emits only the item at `index`, then completes. Completes empty if the
/// source has fewer items.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_iter([10, 20, 30])
///   .element_at(1)
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![20]);
/// ```
#[doc(alias = "elementAt")]
pub type ElementAt<S> = Take<Skip<S>>;

/// Like [`ElementAt`] but emits a default when the source is too short.
pub type ElementAtOr<S, Item> = DefaultIfEmpty<Take<Skip<S>>, Item>;

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_element_at_emits_index() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(0));
    let result_c = result.clone();
    let seen_c = seen.clone();

    Local::from_iter([10, 20, 30, 40])
      .tap(move |_| *seen_c.borrow_mut() += 1)
      .element_at(2)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![30]);
    // Stops pulling after the wanted item
    assert_eq!(*seen.borrow(), 3);
  }

  #[rxrust_macro::test]
  fn test_element_at_out_of_range_completes_empty() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_iter([10, 20])
      .element_at(5)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), Vec::<i32>::new());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_element_at_or_default() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([10, 20])
      .element_at_or(5, -1)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![-1]);
  }

  #[rxrust_macro::test]
  fn test_element_at_or_present() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_iter([10, 20])
      .element_at_or(0, -1)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![10]);
  }
}
