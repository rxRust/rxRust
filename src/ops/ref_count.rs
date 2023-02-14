/// Make a ConnectableObservable behave like a ordinary observable and
/// automates the way you can connect to it.
///
/// Internally it counts the subscriptions to the observable and subscribes
/// (only once) to the source if the number of subscriptions is larger than
/// 0. If the number of subscriptions is smaller than 1, it unsubscribes
/// from the source. This way you can make sure that everything before the
/// published refCount has only a single subscription independently of the
/// number of subscribers to the target observable.
///
/// Note that using the share operator is exactly the same as using the
/// publish operator (making the observable hot) and the refCount operator
/// in a sequence.
use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDerefMut},
};

pub struct ShareOp<'a, Item, Err, Source>(
  MutRc<InnerShareOp<Source, Subject<'a, Item, Err>>>,
);

pub struct ShareOpThreads<Item, Err, Source>(
  MutArc<InnerShareOp<Source, SubjectThreads<Item, Err>>>,
);

enum InnerShareOp<Source, Subject> {
  Connectable(ConnectableObservable<Source, Subject>),
  Connected(Subject),
}

macro_rules! impl_trivial {
  ($name: ident, $rc: ident $(,$lf: lifetime)?) => {
    impl<$($lf,)? Item, Err, S> Clone for $name<$($lf,)? Item, Err, S> {
      fn clone(&self) -> Self {
        Self(self.0.clone())
      }
    }

    impl<$($lf,)? Item, Err, S> $name<$($lf,)? Item, Err, S> {
      #[inline]
      pub fn new(source: S) -> Self {
        let inner = InnerShareOp::Connectable(ConnectableObservable::new(source));
        $name($rc::own(inner))
      }
    }
  };
}

impl_trivial!(ShareOp, MutRc, 'a);
impl_trivial!(ShareOpThreads, MutArc);

macro_rules! impl_observable_methods {
  ($subject: ty) => {
    type Unsub = RefCountSubscription<
      $subject,
      <$subject as Observable<Item, Err, O>>::Unsub,
    >;

    fn actual_subscribe(self, observer: O) -> Self::Unsub {
      let mut inner = self.0.rc_deref_mut();
      match &mut *inner {
        InnerShareOp::Connectable(c) => {
          let subject = c.fork();

          let subscription = subject.clone().actual_subscribe(observer);
          let connected = InnerShareOp::Connected(subject.clone());
          let connectable = std::mem::replace(&mut *inner, connected);

          match connectable {
            InnerShareOp::Connectable(connectable) => connectable.connect(),
            InnerShareOp::Connected { .. } => unreachable!(),
          };

          RefCountSubscription { subject, subscription }
        }
        InnerShareOp::Connected(subject) => {
          let subscription = subject.clone().actual_subscribe(observer);
          RefCountSubscription { subject: subject.clone(), subscription }
        }
      }
    }
  };
}

impl<'a, S, Item, Err, O> Observable<Item, Err, O> for ShareOp<'a, Item, Err, S>
where
  Item: Clone,
  Err: Clone,
  O: Observer<Item, Err> + 'a,
  S: Observable<Item, Err, Subject<'a, Item, Err>>,
{
  impl_observable_methods!(Subject<'a, Item, Err>);
}

impl<'a, S, Item, Err> ObservableExt<Item, Err> for ShareOp<'a, Item, Err, S> where
  S: ObservableExt<Item, Err>
{
}

impl<S, Item, Err, O> Observable<Item, Err, O> for ShareOpThreads<Item, Err, S>
where
  Item: Clone,
  Err: Clone,
  O: Observer<Item, Err> + Send + 'static,
  S: Observable<Item, Err, SubjectThreads<Item, Err>>,
{
  impl_observable_methods!(SubjectThreads< Item, Err>);
}

impl<S, Item, Err> ObservableExt<Item, Err> for ShareOpThreads<Item, Err, S> where
  S: ObservableExt<Item, Err>
{
}
pub struct RefCountSubscription<Subject, U> {
  subject: Subject,
  subscription: U,
}

impl<U, Subject> Subscription for RefCountSubscription<Subject, U>
where
  Subject: Subscription + SubjectSize,
  U: Subscription,
{
  fn unsubscribe(self) {
    self.subscription.unsubscribe();
    if self.subject.is_empty() {
      self.subject.unsubscribe()
    }
  }

  #[inline(always)]
  fn is_closed(&self) -> bool {
    self.subscription.is_closed()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn smoke() {
    let mut accept1 = 0;
    let mut accept2 = 0;
    {
      let ref_count = observable::of(1).share();
      ref_count.clone().subscribe(|v| accept1 = v);
      ref_count.clone().subscribe(|v| accept2 = v);
    }

    assert_eq!(accept1, 1);
    assert_eq!(accept2, 0);
  }

  #[test]
  fn auto_unsubscribe() {
    let mut accept1 = 0;
    let mut accept2 = 0;
    {
      let mut subject = Subject::default();
      let ref_count = subject.clone().share();
      let s1 = ref_count.clone().subscribe(|v| accept1 = v);
      let s2 = ref_count.clone().subscribe(|v| accept2 = v);
      subject.next(1);
      s1.unsubscribe();
      s2.unsubscribe();
      subject.next(2);
    }

    assert_eq!(accept1, 1);
    assert_eq!(accept2, 1);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_ref_count);

  fn bench_ref_count(b: &mut bencher::Bencher) {
    b.iter(smoke)
  }
}
