//! Generate: a lazy state-machine iterator for `ObservableFactory::generate`.

/// Iterator yielding `initial`, then `iterate(&state)` while
/// `condition(&state)` holds.
#[derive(Clone)]
pub struct Generate<T, C, I> {
  state: Option<T>,
  condition: C,
  iterate: I,
}

impl<T, C, I> Generate<T, C, I> {
  /// Creates the generator.
  pub fn new(initial: T, condition: C, iterate: I) -> Self {
    Self { state: Some(initial), condition, iterate }
  }
}

impl<T, C, I> Iterator for Generate<T, C, I>
where
  C: FnMut(&T) -> bool,
  I: FnMut(&T) -> T,
{
  type Item = T;

  fn next(&mut self) -> Option<T> {
    let current = self.state.take()?;
    if !(self.condition)(&current) {
      return None;
    }
    self.state = Some((self.iterate)(&current));
    Some(current)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_generate_counts() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::generate(1, |v| *v <= 4, |v| v + 1).subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn test_generate_empty_when_condition_false() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::generate(0, |_| false, |v| v + 1)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert!(result.borrow().is_empty());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_generate_is_lazy_with_take() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::generate(1u64, |_| true, |v| v * 2)
      .take(5)
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2, 4, 8, 16]);
  }
}
