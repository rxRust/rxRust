use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::observer::observer_proxy_impl;
use crate::prelude::*;

#[derive(Clone)]
pub struct TakeUntilOp<S, T> {
  pub(crate) source: S,
  pub(crate) notifier: T,
}

#[doc(hidden)]
macro observable_impl($subscription:ty, $sharer:path, $mutability_enabler:path, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    // We need to keep a reference to the observer from two places
    let shared_observer = $sharer($mutability_enabler(subscriber.observer));
    // TODO Composite subscription
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

observable_proxy_impl!(TakeUntilOp, S, T);

// TODO Replace T with N (abbreviation for notifier)
impl<'a, S, T> LocalObservable<'a> for TakeUntilOp<S, T>
where
  S: LocalObservable<'a> + 'a,
  T: LocalObservable<'a, Err = S::Err> + 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, Rc::new, RefCell::new, 'a);
}

impl<S, T> SharedObservable for TakeUntilOp<S, T>
where
  S: SharedObservable,
  T: SharedObservable<Err = S::Err>,
  S::Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Arc::new, Mutex::new, Send + Sync + 'static);
}

pub struct TakeUntilObserver<O> {
  observer: O,
}

pub struct TakeUntilNotifierObserver<O, U, Item> {
  // We need access to main observer in order to call `complete` on it as soon
  // as notifier fired
  main_observer: O,
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
  use std::sync::{Arc, Mutex};

  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut last_next_arg = None;
    let mut next_count = 0;
    let mut completed_count = 0;
    {
      let mut notifier = Subject::new();
      let mut source = Subject::new();
      source
        .clone()
        .take_until(notifier.clone())
        .subscribe_complete(
          |i| {
            last_next_arg = Some(i);
            next_count += 1;
          },
          || {
            completed_count += 1;
          },
        );
      source.next(5);
      notifier.next(());
      source.next(6);
      notifier.complete();
      source.complete();
    }
    assert_eq!(next_count, 1);
    assert_eq!(last_next_arg, Some(5));
    assert_eq!(completed_count, 1);
  }

  #[test]
  fn into_shared() {
    let last_next_arg = Arc::new(Mutex::new(None));
    let last_next_arg_mirror = last_next_arg.clone();
    let next_count = Arc::new(Mutex::new(0));
    let next_count_mirror = next_count.clone();
    let completed_count = Arc::new(Mutex::new(0));
    let completed_count_mirror = completed_count.clone();
    let mut notifier = Subject::new();
    let mut source = Subject::new();
    source
      .clone()
      .take_until(notifier.clone())
      .to_shared()
      .subscribe_complete(
        move |i| {
          *last_next_arg.lock().unwrap() = Some(i);
          *next_count.lock().unwrap() += 1;
        },
        move || {
          *completed_count.lock().unwrap() += 1;
        },
      );
    source.next(5);
    notifier.next(());
    source.next(6);
    assert_eq!(*next_count_mirror.lock().unwrap(), 1);
    assert_eq!(*last_next_arg_mirror.lock().unwrap(), Some(5));
    assert_eq!(*completed_count_mirror.lock().unwrap(), 1);
  }
}
