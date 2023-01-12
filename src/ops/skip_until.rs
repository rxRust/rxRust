use std::{
  cell::Cell,
  rc::Rc,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};

use crate::{
  prelude::*,
  rc::{MutArc, MutRc},
};

#[derive(Clone)]
pub struct SkipUntilOp<S, N, NotifyItem, NotifyErr> {
  pub(crate) source: S,
  pub(crate) notifier: N,
  _hint: TypeHint<(NotifyItem, NotifyErr)>,
}

#[derive(Clone)]
pub struct SkipUntilOpThreads<S, N, NotifyItem, NotifyErr> {
  pub(crate) source: S,
  pub(crate) notifier: N,
  _hint: TypeHint<(NotifyItem, NotifyErr)>,
}

macro_rules! impl_skip_until_op {
  ($name: ident, $rc:ident, $observer: ident) => {
    impl<S, N, NotifyItem, NotifyErr> $name<S, N, NotifyItem, NotifyErr> {
      #[inline]
      pub(crate) fn new(source: S, notifier: N) -> Self {
        Self {
          source,
          notifier,
          _hint: TypeHint::default(),
        }
      }
    }

    impl<S, N, Item, Err, O, NotifyItem, NotifyErr> Observable<Item, Err, O>
      for $name<S, N, NotifyItem, NotifyErr>
    where
      O: Observer<Item, Err>,
      S: Observable<Item, Err, $observer<O>>,
      N: Observable<
        NotifyItem,
        NotifyErr,
        SkipUntilNotifierObserver<$observer<O>>,
      >,
    {
      type Unsub = ZipSubscription<S::Unsub, N::Unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        // We need to keep a reference to the observer from two places
        let share_observer = $observer::new(observer);

        let notify_observer = SkipUntilNotifierObserver(share_observer.clone());
        let b = self.notifier.actual_subscribe(notify_observer);
        let a = self.source.actual_subscribe(share_observer);
        ZipSubscription::new(a, b)
      }
    }

    impl<S, N, Item, Err, NotifyItem, NotifyErr> ObservableExt<Item, Err>
      for $name<S, N, NotifyItem, NotifyErr>
    where
      S: ObservableExt<Item, Err>,
      N: ObservableExt<NotifyItem, NotifyErr>,
    {
    }
  };
}

impl_skip_until_op!(SkipUntilOp, MutRc, ShareObserver);
impl_skip_until_op!(SkipUntilOpThreads, MutArc, ShareObserverThreads);

pub struct SkipUntilNotifierObserver<O>(O);

pub struct ShareObserver<O> {
  observer: MutRc<Option<O>>,
  skip: Rc<Cell<bool>>,
}

pub struct ShareObserverThreads<O> {
  observer: MutArc<Option<O>>,
  skip: Arc<AtomicBool>,
}

macro_rules! impl_observer {
  ($name: ident) => {
    impl<Item, Err, O> Observer<Item, Err> for $name<O>
    where
      O: Observer<Item, Err>,
    {
      fn next(&mut self, value: Item) {
        if !self.is_skipping() {
          self.observer.next(value)
        }
      }

      #[inline]
      fn error(self, err: Err) {
        self.observer.error(err)
      }

      #[inline]
      fn complete(self) {
        self.observer.complete()
      }

      #[inline]
      fn is_finished(&self) -> bool {
        self.observer.is_finished()
      }
    }

    impl<O> Clone for $name<O> {
      fn clone(&self) -> Self {
        $name {
          observer: self.observer.clone(),
          skip: self.skip.clone(),
        }
      }
    }

    impl<Item, Err, O> Observer<Item, Err>
      for SkipUntilNotifierObserver<$name<O>>
    {
      #[inline]
      fn next(&mut self, _: Item) {
        self.0.stop_skipping();
      }

      #[inline]
      fn error(self, _: Err) {}

      #[inline]
      fn complete(self) {
        self.0.stop_skipping()
      }

      #[inline]
      fn is_finished(&self) -> bool {
        false
      }
    }
  };
}

impl_observer!(ShareObserver);
impl_observer!(ShareObserverThreads);

impl<O> ShareObserver<O> {
  fn new(observer: O) -> Self {
    Self {
      observer: MutRc::own(Some(observer)),
      skip: Rc::new(Cell::new(true)),
    }
  }

  fn is_skipping(&self) -> bool {
    self.skip.get()
  }

  fn stop_skipping(&self) {
    self.skip.set(false)
  }
}

impl<O> ShareObserverThreads<O> {
  fn new(observer: O) -> Self {
    Self {
      observer: MutArc::own(Some(observer)),
      skip: Arc::new(AtomicBool::new(true)),
    }
  }

  fn is_skipping(&self) -> bool {
    self.skip.load(Ordering::Relaxed)
  }

  fn stop_skipping(&self) {
    self.skip.store(false, Ordering::Relaxed)
  }
}

#[cfg(test)]
mod test {
  use std::vec;

  use crate::prelude::*;

  #[test]

  fn base_function() {
    let mut completed = false;
    let mut items = vec![];
    let mut notifier = Subject::<_, ()>::default();
    let c_notifier = notifier.clone();
    observable::from_iter(0..10)
      .tap(move |v| {
        if v == &5 {
          notifier.next(());
        }
      })
      .skip_until(c_notifier)
      .on_complete(|| completed = true)
      .subscribe(|v| {
        items.push(v);
      });

    assert_eq!(&items, &[5, 6, 7, 8, 9]);
    assert!(completed);
  }

  #[test]
  fn skip_until_support_fork() {
    let mut items1 = vec![];
    let mut items2 = vec![];

    {
      let notifier = Subject::<(), ()>::default();
      let mut c_notifier = notifier.clone();
      let skip_until = observable::from_iter(0..10)
        .tap(move |v| {
          if v == &5 {
            c_notifier.next(())
          }
        })
        .skip_until(notifier);

      skip_until.clone().subscribe(|v| items1.push(v));
      skip_until.subscribe(|v| items2.push(v));
    }

    assert_eq!(items1, items2);
    assert_eq!(&items1, &[5, 6, 7, 8, 9]);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_skip_until);

  fn bench_skip_until(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
