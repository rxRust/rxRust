use crate::prelude::*;
use observable::observable_proxy_impl;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct IgnoreElementsOp<S> {
  pub(crate) source: S,
}

observable_proxy_impl!(IgnoreElementsOp, S);

impl<Item, Err, S, Unsub> LocalObservable<'static> for IgnoreElementsOp<S>
where
  S: LocalObservable<'static, Item = Item, Err = Err, Unsub = Unsub>,
  Unsub: SubscriptionLike + 'static,
  Item: Clone + 'static,
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
      observer: LocalDebounceObserver(Rc::new(RefCell::new(
        IgnoreElementsObserver {
          observer: subscriber.observer,
        },
      ))),
      subscription: subscriber.subscription,
    })
  }
}

impl<S> SharedObservable for IgnoreElementsOp<S>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
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
      observer: SharedIgnoreElementsObserver(Arc::new(Mutex::new(
        IgnoreElementsObserver { observer },
      ))),
      subscription,
    })
  }
}

struct IgnoreElementsObserver<O> {
  observer: O,
}

struct SharedIgnoreElementsObserver<O>(Arc<Mutex<IgnoreElementsObserver<O>>>);
struct LocalDebounceObserver<O>(Rc<RefCell<IgnoreElementsObserver<O>>>);

macro impl_ignore_elements_observer() {
  fn next(&mut self, _: Self::Item) {}
  fn error(&mut self, err: Self::Err) {
    self.0.inner_deref_mut().observer.error(err);
  }
  fn complete(&mut self) { self.0.inner_deref_mut().observer.complete(); }
  fn is_stopped(&self) -> bool { self.0.inner_deref().observer.is_stopped() }
}

impl<O> Observer for SharedIgnoreElementsObserver<O>
where
  O: Observer + Send + 'static,
  O::Item: Clone + Send + 'static,
{
  type Item = O::Item;
  type Err = O::Err;
  impl_ignore_elements_observer!();
}

impl<O> Observer for LocalDebounceObserver<O>
where
  O: Observer + 'static,
  O::Item: Clone + 'static,
{
  type Item = O::Item;
  type Err = O::Err;
  impl_ignore_elements_observer!();
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn smoke() {
    observable::from_iter(0..20)
      .ignore_elements()
      .subscribe(move |_| assert!(false));
  }

  #[test]
  fn shared() {
    observable::from_iter(0..20)
      .ignore_elements()
      .to_shared()
      .subscribe(|_| assert!(false));
  }
}
