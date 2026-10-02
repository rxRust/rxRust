//! FromCallback: emit the values handed to a callback, then complete.

use std::{convert::Infallible, marker::PhantomData};

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// Runs `f` with an emit callback at subscribe; each value passed to the
/// callback is emitted, and the observable completes when `f` returns.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = Vec::new();
/// Local::from_callback(|emit: &mut dyn FnMut(i32)| {
///   emit(1);
///   emit(2);
/// })
/// .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2]);
/// ```
#[doc(alias = "bindCallback")]
pub struct FromCallback<F, Item> {
  pub f: F,
  _marker: PhantomData<fn() -> Item>,
}

impl<F: Clone, Item> Clone for FromCallback<F, Item> {
  fn clone(&self) -> Self { Self { f: self.f.clone(), _marker: PhantomData } }
}

impl<F, Item> FromCallback<F, Item> {
  /// Wraps the callback-taking function.
  pub fn new(f: F) -> Self { Self { f, _marker: PhantomData } }
}

impl<F, Item> ObservableType for FromCallback<F, Item> {
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Infallible;
}

impl<F, Item, C> CoreObservable<C> for FromCallback<F, Item>
where
  C: Context,
  C::Inner: Observer<Item, Infallible>,
  F: FnOnce(&mut dyn FnMut(Item)),
{
  type Unsub = ();

  fn subscribe(self, context: C) -> Self::Unsub {
    let mut observer = context.into_inner();
    (self.f)(&mut |value| {
      if !observer.is_closed() {
        observer.next(value);
      }
    });
    observer.complete();
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_from_callback_emits_and_completes() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));
    let result_c = result.clone();
    let completed_c = completed.clone();

    Local::from_callback(|emit: &mut dyn FnMut(i32)| {
      emit(1);
      emit(2);
    })
    .on_complete(move || *completed_c.borrow_mut() = true)
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_from_callback_respects_take() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::from_callback(|emit: &mut dyn FnMut(i32)| {
      for i in 0..10 {
        emit(i);
      }
    })
    .take(2)
    .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![0, 1]);
  }
}
