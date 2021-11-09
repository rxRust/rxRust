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
use crate::{impl_helper::*, impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct RefCount<R>(R);

pub enum InnerRefCount<Src, Sbj, U> {
  Connectable(ConnectableObservable<Src, Sbj>),
  Connected { subject: Sbj, connection: U },
}

impl<Src, Sbj, U> RefCount<MutRc<InnerRefCount<Src, Sbj, U>>> {
  pub fn local(c: ConnectableObservable<Src, Sbj>) -> Self {
    RefCount(MutRc::own(InnerRefCount::Connectable(c)))
  }
}

impl<Src, Sbj, U> RefCount<MutArc<InnerRefCount<Src, Sbj, U>>> {
  pub fn shared(c: ConnectableObservable<Src, Sbj>) -> Self {
    RefCount(MutArc::own(InnerRefCount::Connectable(c)))
  }
}

impl<Src, Sbj, U> Observable for RefCount<MutRc<InnerRefCount<Src, Sbj, U>>>
where
  Src: Observable,
{
  type Item = Src::Item;
  type Err = Src::Err;
}

impl<Src, Sbj, U> Observable for RefCount<MutArc<InnerRefCount<Src, Sbj, U>>>
where
  Src: Observable,
{
  type Item = Src::Item;
  type Err = Src::Err;
}

impl_local_shared_both! {
  impl<Src, Sbj> RefCount<@ctx::Rc<InnerRefCount<Src, Sbj, Src::Unsub>>>;
  type Unsub = RefCountSubscription<Sbj, Sbj::Unsub, Src::Unsub>;

  macro method($self: ident, $observer: ident, $ctx: ident) {
    let mut inner = $self.0.rc_deref_mut();
    match &mut *inner {
      InnerRefCount::Connectable(c) => {
        let subject = c.fork();
        let subscription = c.fork().actual_subscribe($observer);
        let new_holder : ConnectableObservable<Src, Sbj> = unsafe {
          std::mem::transmute_copy(c)
        };
        let connection = new_holder.connect();
        let old = std::mem::replace(&mut *inner, InnerRefCount::Connected {
          subject: subject.clone(),
          connection: connection.clone()
        });
        std::mem::forget(old);

        RefCountSubscription {
          subject: subject.clone(), subscription, connection
        }
       },
      InnerRefCount::Connected{ subject, connection } => {
        let subscription = subject.clone().actual_subscribe($observer);
        RefCountSubscription {
          subject: subject.clone(),
          subscription,
          connection: connection.clone()
        }
      }
    }
  }
  where
    ConnectableObservable<Src, Sbj>: Connect<Unsub=Src::Unsub>,
    Src: @ctx::Observable @ctx::local_only(+ 'o) @ctx::shared_only(+ 'static),
    Src::Unsub: Clone @ctx::local_only(+ 'o) @ctx::shared_only(+'static),
    Src::Item: Clone  @ctx::local_only(+ 'o) @ctx::shared_only(+'static),
    Src::Err: Clone @ctx::local_only(+ 'o) @ctx::shared_only(+'static),
    Sbj: Observer<Item = Src::Item, Err = Src::Err>
      + TearDownSize + Clone
      + @ctx::Observable<Item=Src::Item, Err=Src::Err>
      @ctx::shared_only(+ Send + Sync + 'static) @ctx::local_only(+ 'o)

}

pub struct RefCountSubscription<S, U, C> {
  subject: S,
  subscription: U,
  connection: C,
}

impl<S, U, C> SubscriptionLike for RefCountSubscription<S, U, C>
where
  S: TearDownSize,
  C: SubscriptionLike,
  U: SubscriptionLike,
{
  fn unsubscribe(&mut self) {
    self.subscription.unsubscribe();
    if self.subject.teardown_size() == 0 {
      self.connection.unsubscribe();
    }
  }

  #[inline(always)]
  fn is_closed(&self) -> bool { self.subscription.is_closed() }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn smoke() {
    let mut accept1 = 0;
    let mut accept2 = 0;
    {
      let ref_count = observable::of(1)
        .publish::<LocalSubject<'_, _, _>>()
        .into_ref_count();
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
      let mut subject = LocalSubject::new();
      let ref_count = subject.clone().publish().into_ref_count();
      let mut s1 = ref_count.clone().subscribe(|v| accept1 = v);
      let mut s2 = ref_count.clone().subscribe(|v| accept2 = v);
      subject.next(1);
      s1.unsubscribe();
      s2.unsubscribe();
      subject.next(2);
    }

    assert_eq!(accept1, 1);
    assert_eq!(accept2, 1);
  }

  #[test]
  fn fork_and_shared() {
    observable::of(1)
      .publish::<LocalSubject<'_, _, _>>()
      .into_ref_count()
      .subscribe(|_| {});

    SharedSubject::new()
      .publish()
      .into_ref_count()
      .into_shared()
      .subscribe(|_: i32| {});

    observable::of(1)
      .publish::<SharedSubject<_, _>>()
      .into_ref_count()
      .into_shared()
      .subscribe(|_| {});

    observable::of(1)
      .into_shared()
      .publish()
      .into_ref_count()
      .into_shared()
      .subscribe(|_| {});
    observable::of(1)
      .into_shared()
      .publish()
      .into_ref_count()
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_ref_count);

  fn bench_ref_count(b: &mut bencher::Bencher) { b.iter(smoke) }
}
