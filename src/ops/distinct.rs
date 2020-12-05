use crate::prelude::*;
use observable::observable_proxy_impl;
use std::{
  cell::RefCell,
  cmp::Eq,
  collections::HashSet,
  hash::Hash,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct DistinctOp<S> {
  pub(crate) source: S,
}

observable_proxy_impl!(DistinctOp, S);

impl<S, Unsub> LocalObservable<'static> for DistinctOp<S>
where
  S: LocalObservable<'static, Unsub = Unsub>,
  Unsub: SubscriptionLike + 'static,
  S::Item: Clone + 'static + Hash + Eq,
{
  type Unsub = Unsub;

  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  >(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let Self { source } = self;
    source.actual_subscribe(Subscriber {
      observer: LocalDinstinctObserver(Rc::new(RefCell::new(
        DistinctObserver {
          observer: subscriber.observer,
          seen: HashSet::new(),
        },
      ))),
      subscription: subscriber.subscription,
    })
  }
}

impl<S> SharedObservable for DistinctOp<S>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static + Eq + Hash,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> S::Unsub {
    let Self { source } = self;
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    source.actual_subscribe(Subscriber {
      observer: SharedDistinctObserver(Arc::new(Mutex::new(
        DistinctObserver {
          observer,
          seen: HashSet::new(),
        },
      ))),
      subscription,
    })
  }
}

struct DistinctObserver<O, Item> {
  observer: O,
  seen: HashSet<Item>,
}

struct SharedDistinctObserver<O, Item>(Arc<Mutex<DistinctObserver<O, Item>>>);
struct LocalDinstinctObserver<O, Item>(Rc<RefCell<DistinctObserver<O, Item>>>);

macro impl_distinct_observer() {
  fn next(&mut self, value: Self::Item) {
    let mut inner = self.0.inner_deref_mut();
    if !inner.seen.contains(&value) {
      inner.seen.insert(value.clone());
      inner.observer.next(value);
    }
  }
  fn error(&mut self, err: Self::Err) {
    let mut inner = self.0.inner_deref_mut();
    inner.observer.error(err);
  }
  fn complete(&mut self) {
    let mut inner = self.0.inner_deref_mut();
    inner.observer.complete();
  }
  fn is_stopped(&self) -> bool {
    let inner = self.0.inner_deref();
    inner.observer.is_stopped()
  }
}

impl<O> Observer for SharedDistinctObserver<O, O::Item>
where
  O: Observer + Send + 'static,
  O::Item: Clone + Send + 'static + Eq + Hash,
{
  type Item = O::Item;
  type Err = O::Err;
  impl_distinct_observer!();
}

impl<O> Observer for LocalDinstinctObserver<O, O::Item>
where
  O: Observer + 'static,
  O::Item: Clone + 'static + Eq + Hash,
{
  type Item = O::Item;
  type Err = O::Err;
  impl_distinct_observer!();
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn smoke() {
    let x = Rc::new(RefCell::new(vec![]));
    let x_c = x.clone();
    observable::from_iter(0..20)
      .map(|v| v % 5)
      .distinct()
      .subscribe(move |v| x.borrow_mut().push(v))
      .unsubscribe();
    assert_eq!(&*x_c.borrow(), &[0, 1, 2, 3, 4]);
  }
  #[test]
  fn shared() {
    observable::from_iter(0..10)
      .distinct()
      .to_shared()
      .to_shared()
      .subscribe(|_| {});
  }
}
