use crate::{
  prelude::*,
  rc::{MutArc, MutRc},
};

#[derive(Clone)]
pub struct TakeUntilOp<S, N, NotifyItem, NotifyErr> {
  source: S,
  notifier: N,
  _hint: TypeHint<(NotifyItem, NotifyErr)>,
}

#[derive(Clone)]
pub struct TakeUntilOpThreads<S, N, NotifyItem, NotifyErr> {
  source: S,
  notifier: N,
  _hint: TypeHint<(NotifyItem, NotifyErr)>,
}

macro_rules! impl_take_until {
  ($name: ident, $rc: ident) => {
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
      S: Observable<Item, Err, $rc<Option<O>>>,
      N: Observable<
        NotifyItem,
        NotifyErr,
        TakeUntilNotifierObserver<Item, Err, $rc<Option<O>>>,
      >,
    {
      type Unsub = ZipSubscription<S::Unsub, N::Unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        // We need to keep a reference to the observer from two places
        let main_observer = $rc::own(Some(observer));

        let a = self.source.actual_subscribe(main_observer.clone());
        let notify_observer = TakeUntilNotifierObserver {
          main_observer,
          _hint: TypeHint::default(),
        };
        let b = self.notifier.actual_subscribe(notify_observer);
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

impl_take_until!(TakeUntilOp, MutRc);
impl_take_until!(TakeUntilOpThreads, MutArc);

pub struct TakeUntilNotifierObserver<Item, Err, O> {
  // We need access to main observer in order to call `complete` on it as soon
  // as notifier fired
  main_observer: O,
  _hint: TypeHint<(Item, Err)>,
}

impl<Item, Err, NotifyItem, NotifyErr, O> Observer<NotifyItem, NotifyErr>
  for TakeUntilNotifierObserver<Item, Err, O>
where
  O: Observer<Item, Err> + Clone,
{
  fn next(&mut self, _: NotifyItem) {
    self.main_observer.clone().complete();
  }

  #[inline]
  fn error(self, _: NotifyErr) {}

  #[inline]
  fn complete(self) {}

  #[inline]
  fn is_finished(&self) -> bool {
    self.main_observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::{
    prelude::*,
    rc::{MutRc, RcDeref, RcDerefMut},
  };

  #[test]
  fn base_function() {
    let mut last_next_arg = None;
    let mut next_count = 0;
    let mut completed_count = 0;
    {
      let mut notifier = Subject::default();
      let mut source = Subject::default();
      source
        .clone()
        .take_until::<_, _, ()>(notifier.clone())
        .on_complete(|| {
          completed_count += 1;
        })
        .subscribe(|i| {
          last_next_arg = Some(i);
          next_count += 1;
        });
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
      let mut notifier = Subject::default();
      let mut source = Subject::default();
      let cloned_source = source.clone();
      let cloned_notifier = notifier.clone();
      notifier
        .clone()
        .on_complete(move || {
          *source_completed_count_cloned.rc_deref_mut() += 1;
        })
        .subscribe(move |j| {
          let last_next_arg = last_next_arg_cloned.clone();
          let next_count = next_count_cloned.clone();
          let notifier_completed_count =
            notifier_completed_count_cloned.clone();
          cloned_source
            .clone()
            .take_until(cloned_notifier.clone())
            .on_complete(move || {
              *notifier_completed_count.rc_deref_mut() += 1;
            })
            .subscribe(move |i| {
              *last_next_arg.rc_deref_mut() = Some((i, j));
              *next_count.rc_deref_mut() += 1;
            });
        });
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
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_take_until);

  fn bench_take_until(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
