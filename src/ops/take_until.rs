use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::observer::observer_proxy_impl;
use crate::prelude::*;

#[derive(Clone)]
pub struct TakeUntilOp<S, T> {
  pub(crate) source: S,
  pub(crate) notifier: T,
}

observable_proxy_impl!(TakeUntilOp, S, T);

impl<'a, S, T> LocalObservable<'a> for TakeUntilOp<S, T>
where
  S: LocalObservable<'a> + 'a,
  T: LocalObservable<'a, Err = S::Err> + 'a,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    // We need to keep a reference to the observer from two places
    let shared_observer = Rc::new(RefCell::new(subscriber.observer));
    let main_subscriber = Subscriber {
      observer: TakeUntilObserver {
        observer: shared_observer.clone(),
      },
      subscription: subscriber.subscription.clone(),
    };
    let notifier_subscriber = Subscriber {
      observer: TakeUntilNotifierObserver {
        main_subscription: subscriber.subscription.clone(),
        main_observer: shared_observer,
        _p: PhantomData,
      },
      subscription: subscriber.subscription,
    };
    self.notifier.actual_subscribe(notifier_subscriber);
    self.source.actual_subscribe(main_subscriber)
  }
}

pub struct TakeUntilObserver<O> {
  observer: Rc<RefCell<O>>,
}

pub struct TakeUntilNotifierObserver<O, U, Item> {
  // We need access to main observer in order to call `complete` on it as soon
  // as notifier fired
  main_observer: Rc<RefCell<O>>,
  // We need to cancel main subscription as soon as notifier fired
  main_subscription: U,
  _p: PhantomData<Item>,
}

impl<O, U, NotifierItem, Item, Err> Observer<NotifierItem, Err>
  for TakeUntilNotifierObserver<O, U, Item>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
{
  fn next(&mut self, _: NotifierItem) {
    self.main_observer.complete();
    self.main_subscription.unsubscribe();
  }

  fn error(&mut self, err: Err) {
    self.main_observer.error(err);
    self.main_subscription.unsubscribe();
  }

  fn complete(&mut self) {
    // Do nothing
  }
}

observer_proxy_impl!(TakeUntilObserver<O>, { observer }, Item, Err, O);

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut last_next_arg = None;
    let mut next_count = 0;
    let mut completed = false;
    {
      let mut notifier: LocalSubject<(), ()> = LocalSubject::new();
      let mut source: LocalSubject<i32, ()> = LocalSubject::new();
      source
        .clone()
        .take_until(notifier.clone())
        .subscribe_complete(
          |i| {
            last_next_arg = Some(i);
            next_count += 1;
          },
          || {
            completed = true;
          },
        );
      source.next(5);
      notifier.next(());
      source.next(6);
    }
    assert_eq!(last_next_arg, Some(5));
    assert_eq!(next_count, 1);
    assert!(completed);
  }
}
