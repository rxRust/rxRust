use crate::observer::error_proxy_impl;
use crate::prelude::*;
use observable::observable_proxy_impl;

#[derive(Clone)]
pub struct DefaultIfEmptyOp<S>
where
  S: Observable,
{
  pub(crate) source: S,
  pub(crate) is_empty: bool,
  pub(crate) default_value: S::Item,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: DefaultIfEmptyObserver {
        observer: subscriber.observer,
        is_empty: self.is_empty,
        default_value: self.default_value,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

observable_proxy_impl!(DefaultIfEmptyOp, S);

impl<'a, S> LocalObservable<'a> for DefaultIfEmptyOp<S>
where
  S: LocalObservable<'a>,
  S::Item: Clone + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<S> SharedObservable for DefaultIfEmptyOp<S>
where
  S: SharedObservable,
  S::Item: Clone + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct DefaultIfEmptyObserver<O, Item> {
  observer: O,
  is_empty: bool,
  default_value: Item,
}

impl<Item, Err, O> Observer<Item, Err> for DefaultIfEmptyObserver<O, Item>
where
  O: Observer<Item, Err>,
  Item: Clone,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value);
    if self.is_empty {
      self.is_empty = false;
    }
  }

  fn complete(&mut self) {
    if self.is_empty {
      self.observer.next(self.default_value.clone());
    }
    self.observer.complete()
  }

  error_proxy_impl!(Err, observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut value = 0;

    observable::of(10)
      .default_if_empty(5)
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 10);
    assert_eq!(completed, true);
  }

  #[test]
  fn base_empty_function() {
    let mut completed = false;
    let mut value = 0;

    observable::empty()
      .default_if_empty(5)
      .subscribe_complete(|v| value = v, || completed = true);

    assert_eq!(value, 5);
    assert_eq!(completed, true);
  }
}
