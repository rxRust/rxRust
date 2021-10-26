use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::prelude::*;

#[derive(Clone)]
pub struct TakeUntilOp<S, N> {
  pub(crate) source: S,
  pub(crate) notifier: N,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $sharer:path, $mutability_enabler:path,
                      $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item, Err= Self::Err> + $($marker +)* $lf {
    let  subscription = subscriber.subscription;
    // We need to keep a reference to the observer from two places
    let shared_observer = $sharer($mutability_enabler(subscriber.observer));
    let main_subscriber = Subscriber {
      observer: shared_observer.clone(),
      subscription: subscription.clone(),
    };
    let notifier_subscriber = Subscriber {
      observer: TakeUntilNotifierObserver {
        subscription: subscription.clone(),
        main_observer: shared_observer,
        _p: TypeHint::new(),
      },
      subscription: subscription.clone(),
    };
    subscription.add(self.notifier.actual_subscribe(notifier_subscriber));
    subscription.add(self.source.actual_subscribe(main_subscriber));
    subscription
  }
}
}

observable_proxy_impl!(TakeUntilOp, S, N);

impl<'a, S, N> LocalObservable<'a> for TakeUntilOp<S, N>
where
  S: LocalObservable<'a> + 'a,
  N: LocalObservable<'a, Err = S::Err> + 'a,
{
  type Unsub = LocalSubscription;
  observable_impl!(LocalSubscription, Rc::new, RefCell::new, 'a);
}

impl<S, N> SharedObservable for TakeUntilOp<S, N>
where
  S: SharedObservable,
  N: SharedObservable<Err = S::Err>,
  S::Item: Send + Sync + 'static,
  N::Item: Send + Sync + 'static,
  S::Unsub: Send + Sync,
  N::Unsub: Send + Sync,
{
  type Unsub = SharedSubscription;
  observable_impl!(
    SharedSubscription,
    Arc::new,
    Mutex::new,
    Send + Sync + 'static
  );
}

pub struct TakeUntilNotifierObserver<O, U, Item> {
  // We need access to main observer in order to call `complete` on it as soon
  // as notifier fired
  main_observer: O,
  // We need to unsubscribe everything as soon as notifier fired
  subscription: U,
  _p: TypeHint<Item>,
}

impl<O, U, NotifierItem, Err> Observer
  for TakeUntilNotifierObserver<O, U, NotifierItem>
where
  O: Observer<Err = Err>,
  U: SubscriptionLike,
{
  type Item = NotifierItem;
  type Err = Err;
  fn next(&mut self, _: NotifierItem) {
    self.main_observer.complete();
    self.subscription.unsubscribe();
  }

  fn error(&mut self, err: Err) {
    self.main_observer.error(err);
    self.subscription.unsubscribe();
  }

  #[inline]
  fn complete(&mut self) { self.subscription.unsubscribe() }
}

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
      let mut notifier = LocalSubject::new();
      let mut source = LocalSubject::new();
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
  fn ininto_shared() {
    let last_next_arg = Arc::new(Mutex::new(None));
    let last_next_arg_mirror = last_next_arg.clone();
    let next_count = Arc::new(Mutex::new(0));
    let next_count_mirror = next_count.clone();
    let completed_count = Arc::new(Mutex::new(0));
    let completed_count_mirror = completed_count.clone();
    let mut notifier = SharedSubject::new();
    let mut source = SharedSubject::new();
    source
      .clone()
      .take_until(notifier.clone())
      .into_shared()
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

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take_until);

  fn bench_take_until(b: &mut bencher::Bencher) { b.iter(base_function); }
}
