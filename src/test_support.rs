//! Deliberately uncooperative source for lifecycle regression tests.
use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::{BoxedObserver, Observer},
  subscription::Subscription,
};
type StoredObservers = Rc<RefCell<Vec<Option<BoxedObserver<'static, i32, &'static str>>>>>;

#[derive(Clone, Default)]
pub(crate) struct Manual {
  pub(crate) initial: Option<i32>,
  observers: StoredObservers,
  cancellations: Rc<RefCell<Vec<Rc<Cell<usize>>>>>,
}
pub(crate) struct ManualSubscription(Rc<Cell<usize>>);
impl Subscription for ManualSubscription {
  fn unsubscribe(self) { self.0.set(self.0.get() + 1); }
  fn is_closed(&self) -> bool { false }
}
impl ObservableType for Manual {
  type Item<'a> = i32;
  type Err = &'static str;
}
impl<C> CoreObservable<C> for Manual
where
  C: Context,
  C::Inner: Observer<i32, &'static str> + 'static,
{
  type Unsub = ManualSubscription;
  fn subscribe(self, context: C) -> Self::Unsub {
    let mut observer = context.into_inner();
    if let Some(value) = self.initial {
      observer.next(value);
    }
    self
      .observers
      .borrow_mut()
      .push(Some(Box::new(observer)));
    let count = Rc::new(Cell::new(0));
    self
      .cancellations
      .borrow_mut()
      .push(count.clone());
    ManualSubscription(count)
  }
}
impl Manual {
  pub(crate) fn subscriptions(&self) -> usize { self.observers.borrow().len() }
  pub(crate) fn cancellations(&self, index: usize) -> usize {
    self.cancellations.borrow()[index].get()
  }
  pub(crate) fn next(&self, index: usize, value: i32) {
    let observer = self.observers.borrow_mut()[index].take();
    if let Some(mut observer) = observer {
      observer.next(value);
      self.observers.borrow_mut()[index] = Some(observer);
    }
  }
  pub(crate) fn complete(&self, index: usize) {
    let observer = self.observers.borrow_mut()[index].take();
    if let Some(observer) = observer {
      observer.complete();
    }
  }
  pub(crate) fn error(&self, index: usize) {
    let observer = self.observers.borrow_mut()[index].take();
    if let Some(observer) = observer {
      observer.error("test failure");
    }
  }
}
