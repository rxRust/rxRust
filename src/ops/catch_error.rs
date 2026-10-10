use std::marker::PhantomData;

use crate::{
  CoreObservable, IntoBoxedSubscription, Subscription,
  context::{Context, RcDerefMut},
  observable::ObservableType,
  observer::Observer,
};

/// The CatchError operator struct.
///
/// # Example
///
/// ```
/// use rxrust::prelude::*;
///
///  let observable = Local::throw_err("some-error")
///    .map(|_| String::new())
///    .catch_error(|error| Local::from_iter([format!("error: {error}"), String::from("after")]));
///
///  let mut result = Vec::new();
///  observable.subscribe(|v| {
///    result.push(v);
///  });
///
///  assert_eq!(result, vec![String::from("error: some-error"), String::from("after")]);
/// ```
pub struct CatchError<S, F> {
  pub source: S,
  pub func: F,
}

pub struct CatchErrorOrigObserver<P, O, F> {
  subscription: P,
  observer: O,
  func: F,
}

impl<NextObserver, Item, OrigErr, F, SubstObservable> Observer<Item, OrigErr>
  for CatchErrorOrigObserver<
    SubstObservable::RcMut<Option<SubstObservable::BoxedSubscription>>,
    NextObserver,
    F,
  >
where
  NextObserver: Observer<Item, <SubstObservable::Inner as ObservableType>::Err>,
  F: FnOnce(OrigErr) -> SubstObservable,
  SubstObservable: Context<
    Inner: CoreObservable<
      SubstObservable::With<NextObserver>,
      Unsub: IntoBoxedSubscription<SubstObservable::BoxedSubscription>,
    >,
  >,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value);
  }

  fn error(self, err: OrigErr) {
    let Self { subscription, observer, func, .. } = self;

    if let Some(sub) = subscription.rc_deref_mut().take() {
      sub.unsubscribe();
    }

    let subst_observable = func(err);
    let subst_subscription = subst_observable
      .into_inner()
      .subscribe(SubstObservable::lift(observer));

    *subscription.rc_deref_mut() = Some(subst_subscription.into_boxed());
  }

  fn complete(self) {
    self.observer.complete();
  }

  fn is_closed(&self) -> bool {
    self.observer.is_closed()
  }
}

pub struct CatchErrorSubstObserver<O> {
  observer: O,
}

impl<O, Item, Err> Observer<Item, Err> for CatchErrorSubstObserver<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    self.observer.next(value)
  }

  fn error(self, err: Err) {
    self.observer.error(err)
  }

  fn complete(self) {
    self.observer.complete();
  }

  fn is_closed(&self) -> bool {
    self.observer.is_closed()
  }
}

impl<S, F, SubstObservable, SubstErr> ObservableType for CatchError<S, F>
where
  S: ObservableType,
  F: FnOnce(S::Err) -> SubstObservable,
  SubstObservable: Context<Inner: ObservableType<Err = SubstErr>>,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = SubstErr;
}

impl<S, F, SubstObservable, SubstErr, Ctx> CoreObservable<Ctx> for CatchError<S, F>
where
  Ctx: Context,
  S: CoreObservable<
    Ctx::With<CatchErrorOrigObserver<Ctx::RcMut<Option<Ctx::BoxedSubscription>>, Ctx::Inner, F>>,
  >,
  S::Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
  F: FnOnce(S::Err) -> SubstObservable,
  SubstObservable: Context<Inner: ObservableType<Err = SubstErr>> + 'static,
  Ctx::RcMut<Option<Ctx::BoxedSubscription>>: Subscription,
{
  type Unsub = Ctx::RcMut<Option<Ctx::BoxedSubscription>>;

  fn subscribe(self, context: Ctx) -> Self::Unsub {
    let Self { source, func } = self;
    let subscription: Ctx::RcMut<_> = Ctx::RcMut::from(None);
    let subscription_clone = subscription.clone();
    let wrapped = context.transform(move |observer| CatchErrorOrigObserver {
      subscription: subscription_clone,
      observer,
      func,
    });
    *subscription.rc_deref_mut() = Some(source.subscribe(wrapped).into_boxed());
    subscription
  }
}
