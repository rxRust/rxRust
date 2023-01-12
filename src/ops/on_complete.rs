use crate::{
  observable::{Observable, ObservableExt},
  observer::Observer,
};

pub struct OnCompleteOp<S, F> {
  pub(crate) source: S,
  pub(crate) func: F,
}

impl<S, F, Item, Err, O> Observable<Item, Err, O> for OnCompleteOp<S, F>
where
  O: Observer<Item, Err>,
  S: Observable<Item, Err, OnCompleteObserver<O, F>>,
  F: FnOnce(),
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(OnCompleteObserver { observer, func: self.func })
  }
}

impl<S, F, Item, Err> ObservableExt<Item, Err> for OnCompleteOp<S, F> where
  S: ObservableExt<Item, Err>
{
}

pub struct OnCompleteObserver<O, F> {
  observer: O,
  func: F,
}

impl<Item, Err, O, F> Observer<Item, Err> for OnCompleteObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnOnce(),
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.observer.next(value)
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(self) {
    (self.func)();
    self.observer.complete();
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}
