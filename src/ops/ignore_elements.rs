use crate::prelude::*;
use observable::observable_proxy_impl;

#[derive(Clone)]
pub struct IgnoreElementsOp<S> {
  pub(crate) source: S,
}

observable_proxy_impl!(IgnoreElementsOp, S);

macro ignore_elements_impl(
  $subscription:ty,
  $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    let subscriber = Subscriber {
      observer: IgnoreElementsObserver {
        observer: subscriber.observer,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

impl<'a, S, Item> LocalObservable<'a> for IgnoreElementsOp<S>
where
  S: LocalObservable<'a, Item = Item>,
  Item: 'a,
{
  type Unsub = S::Unsub;
  ignore_elements_impl!(LocalSubscription,'a);
}

impl<S, Item> SharedObservable for IgnoreElementsOp<S>
where
  S: SharedObservable<Item = Item>,
  Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  ignore_elements_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct IgnoreElementsObserver<O> {
  observer: O,
}

impl<O, Item, Err> Observer for IgnoreElementsObserver<O>
where
  O: Observer<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, _value: Self::Item) {}
  fn complete(&mut self) { self.observer.complete(); }
  fn error(&mut self, err: Self::Err) { self.observer.error(err); }
  fn is_stopped(&self) -> bool { self.observer.is_stopped() }
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
