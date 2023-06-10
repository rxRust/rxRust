use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};

#[derive(Clone)]
pub struct MergeOp<S1, S2> {
  source1: S1,
  source2: S2,
}
#[derive(Clone)]
pub struct MergeOpThreads<S1, S2> {
  source1: S1,
  source2: S2,
}

macro_rules! impl_merge_op {
  ($name: ident, $rc: ident) => {
    impl<S1, S2> $name<S1, S2> {
      #[inline]
      pub fn new(source1: S1, source2: S2) -> Self {
        $name { source1, source2 }
      }
    }

    impl<S1, S2, Item, Err, O> Observable<Item, Err, O> for $name<S1, S2>
    where
      O: Observer<Item, Err>,
      S1: Observable<Item, Err, $rc<MergeObserver<O>>>,
      S2: Observable<Item, Err, $rc<MergeObserver<O>>>,
    {
      type Unsub = ZipSubscription<S1::Unsub, S2::Unsub>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let observer = MergeObserver {
          observer: Some(observer),
          completed_one: false,
        };
        let observer = $rc::own(observer);
        let a = self.source1.actual_subscribe(observer.clone());
        let b = self.source2.actual_subscribe(observer.clone());
        ZipSubscription::new(a, b)
      }
    }

    impl<S1, S2, Item, Err> ObservableExt<Item, Err> for $name<S1, S2>
    where
      S1: ObservableExt<Item, Err>,
      S2: ObservableExt<Item, Err>,
    {
    }

    impl<Item, Err, O> Observer<Item, Err> for $rc<MergeObserver<O>>
    where
      O: Observer<Item, Err>,
    {
      fn next(&mut self, value: Item) {
        let mut inner = self.rc_deref_mut();
        if let Some(observer) = inner.observer.as_mut() {
          observer.next(value)
        }
      }

      fn error(self, err: Err) {
        let mut inner = self.rc_deref_mut();
        if let Some(o) = inner.observer.take() {
          o.error(err)
        }
      }

      fn complete(self) {
        let mut inner = self.rc_deref_mut();
        if !inner.completed_one {
          inner.completed_one = true;
        } else {
          if let Some(o) = inner.observer.take() {
            o.complete()
          }
        }
      }

      fn is_finished(&self) -> bool {
        self
          .rc_deref()
          .observer
          .as_ref()
          .map_or(true, |o| o.is_finished())
      }
    }
  };
}

impl_merge_op!(MergeOp, MutRc);
impl_merge_op!(MergeOpThreads, MutArc);

pub struct MergeObserver<O> {
  observer: Option<O>,
  completed_one: bool,
}

#[cfg(test)]
mod test {
  use crate::{
    prelude::*,
    rc::{MutArc, RcDeref, RcDerefMut},
  };
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn odd_even_merge() {
    // three collection to store streams emissions
    let mut odd_store = vec![];
    let mut even_store = vec![];
    let mut numbers_store = vec![];

    {
      let mut numbers = Subject::default();
      // enabling multiple observers for even stream;
      let even = numbers.clone().filter(|v| *v % 2 == 0);
      // enabling multiple observers for odd stream;
      let odd = numbers.clone().filter(|v| *v % 2 != 0);

      // merge odd and even stream again
      let merged = even.clone().merge(odd.clone());

      //  attach observers
      merged.subscribe(|v| numbers_store.push(v));
      odd.subscribe(|v| odd_store.push(v));
      even.subscribe(|v| even_store.push(v));

      (0..10).for_each(|v| {
        numbers.next(v);
      });
    }
    assert_eq!(even_store, vec![0, 2, 4, 6, 8]);
    assert_eq!(odd_store, vec![1, 3, 5, 7, 9]);
    assert_eq!(numbers_store, (0..10).collect::<Vec<_>>());
  }

  #[test]
  fn merge_unsubscribe_work() {
    let mut numbers = Subject::default();
    // enabling multiple observers for even stream;
    let even = numbers.clone().filter(|v| *v % 2 == 0);
    // enabling multiple observers for odd stream;
    let odd = numbers.clone().filter(|v| *v % 2 != 0);

    even
      .merge(odd)
      .subscribe(|_| unreachable!("oh, unsubscribe not work."))
      .unsubscribe();

    numbers.next(&1);
  }

  #[test]
  fn completed_test() {
    let completed = Arc::new(AtomicBool::new(false));
    let c_clone = completed.clone();
    let even = Subject::default();
    let odd = Subject::default();

    even
      .clone()
      .merge(odd.clone())
      .on_complete(move || completed.store(true, Ordering::Relaxed))
      .subscribe(|_: &()| {});

    even.clone().complete();
    assert!(!c_clone.load(Ordering::Relaxed));
    odd.complete();
    assert!(c_clone.load(Ordering::Relaxed));
    c_clone.store(false, Ordering::Relaxed);
    even.complete();
    assert!(!c_clone.load(Ordering::Relaxed));
  }

  #[test]
  fn error_test() {
    let completed = Arc::new(Mutex::new(0));
    let cc = completed.clone();
    let error = Arc::new(Mutex::new(0));
    let ec = error.clone();
    let even = Subject::default();
    let odd = Subject::default();

    even
      .clone()
      .merge(odd.clone())
      .on_complete(move || *completed.lock().unwrap() += 1)
      .on_error(move |_| *error.lock().unwrap() += 1)
      .subscribe(|_: ()| {});

    odd.error("");
    even.clone().error("");
    even.complete();

    // if error occur,  stream terminated.
    assert_eq!(*cc.lock().unwrap(), 0);
    // error should be hit just once
    assert_eq!(*ec.lock().unwrap(), 1);
  }

  #[test]
  fn merge_fork() {
    let o = observable::create(|mut s: Subscriber<_>| {
      s.next(1);
      s.next(2);
    });

    let m = o.clone().merge(o);
    let values = MutArc::own(vec![]);

    {
      m.clone().merge(m).subscribe(|x| {
        values.rc_deref_mut().push(x);
      });
    }

    assert_eq!(values.rc_deref().clone(), vec![1, 2, 1, 2, 1, 2, 1, 2]);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_merge);

  fn bench_merge(b: &mut bencher::Bencher) {
    b.iter(odd_even_merge);
  }
}
