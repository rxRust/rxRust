use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDerefMut},
};

#[derive(Clone)]
pub struct SampleOp<Source, Sample, SampleItem> {
  source: Source,
  sample: Sample,
  _hint: TypeHint<SampleItem>,
}

#[derive(Clone)]
pub struct SampleOpThreads<Source, Sample, SampleItem> {
  source: Source,
  sample: Sample,
  _hint: TypeHint<SampleItem>,
}

macro_rules! impl_sample_op {
  ($name: ident, $rc: ident) => {
    impl<Source, Sample, SampleItem> $name<Source, Sample, SampleItem> {
      #[inline]
      pub(crate) fn new(source: Source, sample: Sample) -> Self {
        Self {
          source,
          sample,
          _hint: TypeHint::default(),
        }
      }
    }

    impl<Item1, Item2, Err, Source, Sample, O> Observable<Item1, Err, O>
      for $name<Source, Sample, Item2>
    where
      O: Observer<Item1, Err>,
      Source: Observable<
        Item1,
        Err,
        SourceObserver<$rc<Option<O>>, $rc<Option<Item1>>>,
      >,
      Sample: Observable<
        Item2,
        Err,
        SampleObserver<$rc<Option<O>>, $rc<Option<Item1>>>,
      >,
    {
      type Unsub = ZipSubscription<Source::Unsub, Sample::Unsub>;
      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let value = $rc::own(None);
        let observer = $rc::own(Some(observer));
        let source_observer = SourceObserver {
          observer: observer.clone(),
          value: value.clone(),
        };
        let sample_observer = SampleObserver { observer, value };

        let source_unsub = self.source.actual_subscribe(source_observer);
        let sample_unsub = self.sample.actual_subscribe(sample_observer);
        ZipSubscription::new(source_unsub, sample_unsub)
      }
    }

    impl<Item1, Item2, Err, Source, Sample> ObservableExt<Item1, Err>
      for $name<Source, Sample, Item2>
    where
      Source: ObservableExt<Item1, Err>,
      Sample: ObservableExt<Item2, Err>,
    {
    }
  };
}

impl_sample_op!(SampleOp, MutRc);
impl_sample_op!(SampleOpThreads, MutArc);

pub struct SourceObserver<O, V> {
  observer: O,
  value: V,
}

impl<Item, Err, O, V> Observer<Item, Err> for SourceObserver<O, V>
where
  O: Observer<Item, Err>,
  V: RcDerefMut<Target = Option<Item>>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    *self.value.rc_deref_mut() = Some(value);
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

pub struct SampleObserver<O, V> {
  observer: O,
  value: V,
}

impl<Item1, Item2, V, Err, O> Observer<Item2, Err> for SampleObserver<O, V>
where
  O: Observer<Item1, Err>,
  V: RcDerefMut<Target = Option<Item1>>,
{
  fn next(&mut self, _: Item2) {
    if let Some(item) = self.value.rc_deref_mut().take() {
      self.observer.next(item)
    }
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn complete(mut self) {
    if let Some(item) = self.value.rc_deref_mut().take() {
      self.observer.next(item)
    }
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod test {
  use crate::{observable::fake_timer::FakeClock, prelude::*};
  use std::{cell::RefCell, rc::Rc, time::Duration};

  #[test]
  fn sample_base() {
    let clock = FakeClock::default();
    let x = Rc::new(RefCell::new(vec![]));

    let interval = clock.interval(Duration::from_millis(1));
    {
      let x_c = x.clone();
      interval
        .sample(clock.interval(Duration::from_millis(10)))
        .subscribe(move |v| {
          x_c.borrow_mut().push(v);
        });

      clock.advance(Duration::from_millis(101));
      assert_eq!(&*x.borrow(), &[9, 19, 29, 39, 49, 59, 69, 79, 89, 99]);
    };
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn sample_by_subject() {
    use crate::rc::{MutArc, RcDeref, RcDerefMut};

    let mut subject = SubjectThreads::default();
    let mut notifier = SubjectThreads::default();
    let test_code = MutArc::own(0);
    let c_test_code = test_code.clone();
    subject.clone().sample_threads(notifier.clone()).subscribe(
      move |v: i32| {
        *c_test_code.rc_deref_mut() = v;
      },
    );
    subject.next(1);
    notifier.next(1);
    assert_eq!(*test_code.rc_deref(), 1);

    subject.next(2);
    assert_eq!(*test_code.rc_deref(), 1);

    subject.next(3);
    notifier.next(1);
    assert_eq!(*test_code.rc_deref(), 3);

    *test_code.rc_deref_mut() = 0;
    notifier.next(1);
    assert_eq!(*test_code.rc_deref(), 0);

    subject.next(4);
    notifier.complete();
    assert_eq!(*test_code.rc_deref(), 4);
  }
}
