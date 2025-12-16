use std::marker::PhantomData;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::{Emitter, Observer},
  subscription::Subscription,
};

/// Observable created from a function.
///
/// This struct is created by `ObservableFactory::create`.
#[derive(Clone)]
pub struct Create<F, Item, Err> {
  f: F,
  _marker: PhantomData<(Item, Err)>,
}

impl<F, Item, Err> Create<F, Item, Err> {
  pub fn new(f: F) -> Self { Self { f, _marker: PhantomData } }
}

impl<F, Item, Err> ObservableType for Create<F, Item, Err> {
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Err;
}

/// Wrapper to implement Emitter for Option<O>
struct CreateEmitter<O>(Option<O>);

impl<O, Item, Err> Emitter<Item, Err> for CreateEmitter<O>
where
  O: Observer<Item, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    if let Some(observer) = &mut self.0 {
      observer.next(value);
    }
  }

  #[inline]
  fn error(&mut self, err: Err) {
    if let Some(observer) = self.0.take() {
      observer.error(err);
    }
  }

  #[inline]
  fn complete(&mut self) {
    if let Some(observer) = self.0.take() {
      observer.complete();
    }
  }
}

impl<C, F, Item, Err, U> CoreObservable<C> for Create<F, Item, Err>
where
  C: Context,
  C::Inner: Observer<Item, Err>,
  F: FnOnce(&mut dyn Emitter<Item, Err>) -> U,
  U: Subscription,
{
  type Unsub = U;

  fn subscribe(self, context: C) -> Self::Unsub {
    let observer = context.into_inner();
    let mut emitter = CreateEmitter(Some(observer));
    (self.f)(&mut emitter)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::{prelude::*, subscription::ClosureSubscription};

  #[rxrust_macro::test]
  fn test_create_next_complete() {
    let emitted = Rc::new(RefCell::new(vec![]));
    let emitted_clone = emitted.clone();

    Local::create(|emitter| {
      emitter.next(1);
      emitter.next(2);
      emitter.complete();
    })
    .subscribe(move |v| emitted_clone.borrow_mut().push(v));

    assert_eq!(*emitted.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test]
  fn test_create_error() {
    let error = Rc::new(RefCell::new(None));
    let error_clone = error.clone();

    Local::create(|emitter| {
      emitter.error("oops");
    })
    .on_error(move |e| *error_clone.borrow_mut() = Some(e))
    .subscribe(|_: ()| {});

    assert_eq!(*error.borrow(), Some("oops"));
  }

  #[rxrust_macro::test]
  fn test_create_teardown() {
    let unsubscribed = Rc::new(RefCell::new(false));
    let unsub_clone = unsubscribed.clone();

    let subscription = Local::create(move |emitter| {
      emitter.next(1);
      // Return a closure wrapped in ClosureSubscription
      ClosureSubscription(move || *unsub_clone.borrow_mut() = true)
    })
    .subscribe(|_| {});

    assert!(!*unsubscribed.borrow());
    subscription.unsubscribe();
    assert!(*unsubscribed.borrow());
  }
}
