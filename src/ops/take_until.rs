use crate::impl_helper::impl_local::actual_subscribe;
use crate::{impl_helper::*, impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct TakeUntilOp<S, N> {
  pub(crate) source: S,
  pub(crate) notifier: N,
}

impl<S: Observable, N> Observable for TakeUntilOp<S, N> {
  type Item = S::Item;
  type Err = S::Err;
}
impl<'o, S, N> LocalObservable<'o> for TakeUntilOp<S, N>
where
  S: LocalObservable<'o>,
  N: LocalObservable<'o, Err = S::Err> + 'o,
  S::Item: 'o,
  S::Err: 'o,
  S::Unsub: 'static,
  N::Unsub: 'static,
{
  type Unsub = impl_local::RcMultiSubscription;
  #[allow(unused_mut)]
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'o,
  {
    {
      let subscription = LocalSubscription::default();
      let shared_observer = BufferedMutRc::own(observer);
      subscription.add(self.notifier.actual_subscribe(
        TakeUntilNotifierObserver {
          subscription: subscription.clone(),
          main_observer: shared_observer.clone(),
          _p: TypeHint::new(),
        },
      ));
      subscription.add(self.source.actual_subscribe(shared_observer));
      subscription
    }
  }
}

impl<S, N> SharedObservable for TakeUntilOp<S, N>
where
  S: SharedObservable,
  N: SharedObservable<Err = S::Err>,
  N::Item: 'static,
  S::Unsub: 'static,
  N::Unsub: 'static,
{
  type Unsub = impl_shared::RcMultiSubscription;
  #[allow(unused_mut)]
  fn actual_subscribe<O>(self, mut _observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    {
      let subscription = impl_shared::RcMultiSubscription::default();
      let shared_observer = impl_shared::Rc::own(_observer);
      subscription.add(self.notifier.actual_subscribe(
        TakeUntilNotifierObserver {
          subscription: subscription.clone(),
          main_observer: shared_observer.clone(),
          _p: TypeHint::new(),
        },
      ));
      subscription.add(self.source.actual_subscribe(shared_observer));
      subscription
    }
  }
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
  fn circular_next() {
    let last_next_arg = MutRc::own(None);
    let next_count = MutRc::own(0);
    let source_completed_count = MutRc::own(0);
    let notifier = LocalSubject::new();
    let mut source = LocalSubject::new();

    {
      let source_completed_count = source_completed_count.clone();
      let mut notifier = notifier.clone();
      let last_next_arg = last_next_arg.clone();
      let next_count = next_count.clone();

      source
        .clone()
        .take_until(notifier.clone())
        .subscribe_complete(
          move |i| {
            *last_next_arg.rc_deref_mut() = Some(i);
            *next_count.rc_deref_mut() += 1;
            if i > 2 {
              notifier.next(());
            }
          },
          move || {
            *source_completed_count.rc_deref_mut() += 1;
          },
        );
      source.next(1);
      source.next(3);
      source.next(5);
    }
    assert_eq!(*next_count.rc_deref(), 2);
    assert_eq!(*last_next_arg.rc_deref(), Some(3));
    assert_eq!(*source_completed_count.rc_deref(), 1);
  }

  #[test]
  fn circular() {
    let last_next_arg = MutRc::own(None);
    let next_count = MutRc::own(0);
    let last_next_arg_cloned = last_next_arg.clone();
    let next_count_cloned = next_count.clone();
    let source_completed_count = MutRc::own(0);
    let source_completed_count_cloned = source_completed_count.clone();
    let notifier_completed_count = MutRc::own(0);
    let notifier_completed_count_cloned = notifier_completed_count.clone();
    {
      let mut notifier = LocalSubject::new();
      let mut source = LocalSubject::new();
      let cloned_source = source.clone();
      let cloned_notifier = notifier.clone();
      notifier.clone().subscribe_complete(
        move |j| {
          let last_next_arg = last_next_arg_cloned.clone();
          let next_count = next_count_cloned.clone();
          let notifier_completed_count =
            notifier_completed_count_cloned.clone();
          cloned_source
            .clone()
            .take_until(cloned_notifier.clone())
            .subscribe_complete(
              move |i| {
                *last_next_arg.rc_deref_mut() = Some((i, j));
                *next_count.rc_deref_mut() += 1;
              },
              move || {
                *notifier_completed_count.rc_deref_mut() += 1;
              },
            );
        },
        move || {
          *source_completed_count_cloned.rc_deref_mut() += 1;
        },
      );
      source.next(5);
      notifier.next(1);
      source.next(6);
      notifier.next(2);
      source.next(7);
      notifier.complete();
      source.complete();
    }
    assert_eq!(*next_count.rc_deref(), 2);
    assert_eq!(*last_next_arg.rc_deref(), Some((7, 2)));
    assert_eq!(*source_completed_count.rc_deref(), 1);
    assert_eq!(*notifier_completed_count.rc_deref(), 2);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take_until);

  fn bench_take_until(b: &mut bencher::Bencher) { b.iter(base_function); }
}
