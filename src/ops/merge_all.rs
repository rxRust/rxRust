use super::box_it::{LocalBoxOp, SharedBoxOp};
use crate::prelude::*;
use std::{
  cell::RefCell,
  collections::VecDeque,
  rc::Rc,
  sync::{Arc, Mutex},
};

pub struct MergeAllOp<S> {
  pub concurrent: usize,
  pub source: S,
}

impl<S> Observable for MergeAllOp<S>
where
  S: Observable,
  S::Item: Observable,
{
  type Item = <S::Item as Observable>::Item;
  type Err = S::Err;
}

impl<'a, S> LocalObservable<'a> for MergeAllOp<S>
where
  S: LocalObservable<'a>,
  S::Item: LocalObservable<'a, Err = S::Err> + 'a,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<O: Observer<Item = Self::Item, Err = Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    self
      .source
      .map(|v| v.box_it())
      .actual_subscribe(Subscriber::local(Rc::new(RefCell::new(
        LocalMergeAllObserver {
          observer: subscriber.observer,
          subscribed: 0,
          concurrent: self.concurrent,
          subscription: subscriber.subscription,
          buffer: <_>::default(),
          completed: false,
        },
      ))))
  }
}

pub struct LocalMergeAllObserver<'a, O: Observer> {
  observer: O,
  subscribed: usize,
  concurrent: usize,
  subscription: LocalSubscription,
  completed: bool,
  buffer: VecDeque<LocalBoxOp<'a, O::Item, O::Err>>,
}

impl<'a, O> Observer for Rc<RefCell<LocalMergeAllObserver<'a, O>>>
where
  O: Observer + 'a,
{
  type Item = LocalBoxOp<'a, O::Item, O::Err>;
  type Err = O::Err;

  fn next(&mut self, value: Self::Item) {
    let mut inner = self.borrow_mut();
    if inner.subscribed < inner.concurrent {
      inner.subscription.add(
        value.actual_subscribe(Subscriber::local(LocalInnerObserver(
          self.clone(),
        ))),
      );
      inner.subscribed += 1;
    } else {
      inner.buffer.push_back(value);
    }
  }

  fn error(&mut self, err: Self::Err) {
    let mut inner = self.borrow_mut();
    inner.completed = true;
    inner.observer.error(err);
    inner.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    let mut inner = self.borrow_mut();
    inner.completed = true;
    if inner.subscribed == 0 && inner.buffer.is_empty() {
      inner.observer.complete()
    }
  }
}

struct LocalInnerObserver<'a, O: Observer>(
  Rc<RefCell<LocalMergeAllObserver<'a, O>>>,
);

impl<'a, O> Observer for LocalInnerObserver<'a, O>
where
  O: Observer + 'a,
{
  type Item = O::Item;
  type Err = O::Err;
  #[inline]
  fn next(&mut self, value: Self::Item) {
    self.0.borrow_mut().observer.next(value);
  }

  fn error(&mut self, err: Self::Err) {
    let mut inner = self.0.borrow_mut();
    inner.subscribed -= 1;
    inner.observer.error(err);
    inner.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    let mut inner = self.0.borrow_mut();

    if let Some(o) = inner.buffer.pop_front() {
      inner.subscription.add(o.actual_subscribe(Subscriber::local(
        LocalInnerObserver(self.0.clone()),
      )));
    } else {
      inner.subscribed -= 1;
      if inner.completed && inner.subscribed == 0 {
        inner.observer.complete();
        inner.subscription.unsubscribe();
      }
    }
  }
}

impl<S> SharedObservable for MergeAllOp<S>
where
  S: SharedObservable,
  S::Err: Send + Sync + 'static,
  S::Item: SharedObservable<Err = S::Err> + Send + Sync + 'static,
  <S::Item as SharedObservable>::Unsub: Send + Sync + 'static,
  Self::Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    self
      .source
      .map(|v| v.box_it())
      .actual_subscribe(Subscriber::shared(Arc::new(Mutex::new(
        SharedMergeAllObserver {
          observer: subscriber.observer,
          subscribed: 0,
          concurrent: self.concurrent,
          subscription: subscriber.subscription,
          buffer: <_>::default(),
          completed: false,
        },
      ))))
  }
}

pub struct SharedMergeAllObserver<O: Observer> {
  observer: O,
  subscribed: usize,
  concurrent: usize,
  subscription: SharedSubscription,
  completed: bool,
  buffer: VecDeque<SharedBoxOp<O::Item, O::Err>>,
}

impl<O> Observer for Arc<Mutex<SharedMergeAllObserver<O>>>
where
  O: Observer + Send + Sync + 'static,
{
  type Item = SharedBoxOp<O::Item, O::Err>;
  type Err = O::Err;

  fn next(&mut self, value: Self::Item) {
    let mut inner = self.lock().unwrap();
    if inner.subscribed < inner.concurrent {
      inner
        .subscription
        .add(
          value.actual_subscribe(Subscriber::shared(SharedInnerObserver(
            self.clone(),
          ))),
        );
      inner.subscribed += 1;
    } else {
      inner.buffer.push_back(value);
    }
  }

  fn error(&mut self, err: Self::Err) {
    let mut inner = self.lock().unwrap();
    inner.completed = true;
    inner.observer.error(err);
    inner.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    let mut inner = self.lock().unwrap();
    inner.completed = true;
    if inner.subscribed == 0 && inner.buffer.is_empty() {
      inner.observer.complete()
    }
  }
}

struct SharedInnerObserver<O: Observer>(Arc<Mutex<SharedMergeAllObserver<O>>>);

impl<O> Observer for SharedInnerObserver<O>
where
  O: Observer + Send + Sync + 'static,
{
  type Item = O::Item;
  type Err = O::Err;
  #[inline]
  fn next(&mut self, value: Self::Item) {
    self.0.lock().unwrap().observer.next(value);
  }

  fn error(&mut self, err: Self::Err) {
    let mut inner = self.0.lock().unwrap();
    inner.subscribed -= 1;
    inner.observer.error(err);
    inner.subscription.unsubscribe();
  }

  fn complete(&mut self) {
    let mut inner = self.0.lock().unwrap();

    if let Some(o) = inner.buffer.pop_front() {
      inner
        .subscription
        .add(o.actual_subscribe(Subscriber::shared(SharedInnerObserver(
          self.0.clone(),
        ))));
    } else {
      inner.subscribed -= 1;
      if inner.completed && inner.subscribed == 0 {
        inner.observer.complete();
        inner.subscription.unsubscribe();
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::observable::SubscribeBlocking;
  use futures::executor::{LocalPool, ThreadPool};
  use std::time::Duration;

  #[test]
  fn local() {
    let values = Rc::new(RefCell::new(vec![]));
    let c_values = values.clone();

    let mut local = LocalPool::new();
    observable::from_iter(
      (0..3)
        .map(|_| interval(Duration::from_millis(1), local.spawner()).take(5)),
    )
    .merge_all(2)
    .subscribe(move |i| values.borrow_mut().push(i));
    local.run();

    assert_eq!(
      &*c_values.borrow(),
      &[0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4]
    );
  }

  #[test]
  fn shared() {
    let values = Arc::new(Mutex::new(vec![]));
    let c_values = values.clone();

    let pool = ThreadPool::new().unwrap();
    observable::from_iter(
      (0..3).map(|_| interval(Duration::from_millis(1), pool.clone()).take(5)),
    )
    .merge_all(2)
    .into_shared()
    .subscribe_blocking(move |i| values.lock().unwrap().push(i));

    assert_eq!(
      &*c_values.lock().unwrap(),
      &[0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4]
    );
  }
}
