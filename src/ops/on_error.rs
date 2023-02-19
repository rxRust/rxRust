use std::{convert::Infallible, marker::PhantomData};

use crate::{
  observable::{Observable, ObservableExt},
  observer::Observer,
};

pub struct OnErrorOp<S, F, Err> {
  pub(crate) source: S,
  pub(crate) func: F,
  _marker: PhantomData<Err>,
}

impl<S, F, Err> OnErrorOp<S, F, Err> {
  pub fn new(source: S, func: F) -> Self {
    OnErrorOp { source, func, _marker: PhantomData }
  }
}

impl<S, F, Item, Err, O> Observable<Item, Infallible, O>
  for OnErrorOp<S, F, Err>
where
  O: Observer<Item, Infallible>,
  S: Observable<Item, Err, OnErrorObserver<O, F>>,
  F: FnOnce(Err),
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(OnErrorObserver { observer, func: self.func })
  }
}

impl<S, F, Item, Err> ObservableExt<Item, Infallible> for OnErrorOp<S, F, Err> where
  S: ObservableExt<Item, Err>
{
}

pub struct OnErrorObserver<O, F> {
  observer: O,
  func: F,
}

impl<Item, Err, O, F> Observer<Item, Err> for OnErrorObserver<O, F>
where
  O: Observer<Item, Infallible>,
  F: FnOnce(Err),
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next(value)
  }

  #[inline]
  fn error(self, err: Err) {
    (self.func)(err);
  }

  #[inline]
  fn complete(self) {
    self.observer.complete();
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}
