//! Emits source values until the notifier emits. Notifier errors and completion
//! are ignored; source termination releases an active notifier.
use crate::{
  context::{Context, RcDeref, RcDerefMut, SharedCell},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, TupleSubscription, single_assignment::State},
};
#[derive(Clone)]
pub struct TakeUntil<S, N> {
  pub source: S,
  pub notifier: N,
}
impl<S: ObservableType, N> ObservableType for TakeUntil<S, N> {
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}
pub struct TakeUntilObserver<O, H, F> {
  observer: O,
  notifier: H,
  notifier_done: F,
}
pub struct TakeUntilNotifierObserver<O, H, F, T> {
  observer: O,
  upstreams: Option<H>,
  done: F,
  complete: fn(T),
}
type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type Down<C> = <C as Context>::RcMut<Option<<C as Context>::Inner>>;
impl<S, N, C, U, V> CoreObservable<C> for TakeUntil<S, N>
where
  C: Context,
  U: Subscription,
  V: Subscription,
  S: CoreObservable<C::With<TakeUntilObserver<Down<C>, (), C::RcCell<bool>>>, Unsub = U>
    + CoreObservable<C::With<TakeUntilObserver<Down<C>, Handle<C, V>, C::RcCell<bool>>>, Unsub = U>,
  N: CoreObservable<
      C::With<TakeUntilNotifierObserver<Down<C>, (), C::RcCell<bool>, C::Inner>>,
      Unsub = V,
    > + CoreObservable<
      C::With<
        TakeUntilNotifierObserver<
          Down<C>,
          TupleSubscription<Handle<C, U>, Handle<C, V>>,
          C::RcCell<bool>,
          C::Inner,
        >,
      >,
      Unsub = V,
    >,
  for<'a> C::Inner: Observer<S::Item<'a>, S::Err>,
{
  type Unsub = TupleSubscription<Handle<C, U>, Handle<C, V>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let (downstream, scheduler) = context.into_parts();
    let observer = C::RcMut::from(Some(downstream));
    let (install_source, [source, source_subscription]) = Handle::<C, U>::channel();
    let (install_notifier, [notifier, source_notifier, notifier_subscription]) =
      Handle::<C, V>::channel();
    let done = C::RcCell::from(false);
    let n = TakeUntilNotifierObserver {
      observer: observer.clone(),
      upstreams: Some(TupleSubscription::new(source, notifier)),
      done: done.clone(),
      complete: |downstream: C::Inner| downstream.complete(),
    };
    install_notifier(
      self
        .notifier
        .subscribe(C::With::from_parts(n, scheduler.clone())),
    );
    if observer.rc_deref().is_some() {
      install_source(self.source.subscribe(C::With::from_parts(
        TakeUntilObserver { observer, notifier: source_notifier, notifier_done: done },
        scheduler,
      )));
    }
    TupleSubscription::new(source_subscription, notifier_subscription)
  }
}
impl<I, E, O, H, F, T> Observer<I, E> for TakeUntilObserver<O, H, F>
where
  O: RcDerefMut<Target = Option<T>>,
  T: Observer<I, E>,
  H: Subscription,
  F: SharedCell<bool>,
{
  fn next(&mut self, value: I) {
    if let Some(observer) = self.observer.rc_deref_mut().as_mut() {
      observer.next(value);
    }
  }
  fn error(self, e: E) {
    let observer = { self.observer.rc_deref_mut().take() };
    if let Some(observer) = observer {
      observer.error(e);
      if !self.notifier_done.get() {
        self.notifier.unsubscribe();
      }
    }
  }
  fn complete(self) {
    let observer = { self.observer.rc_deref_mut().take() };
    if let Some(observer) = observer {
      observer.complete();
      if !self.notifier_done.get() {
        self.notifier.unsubscribe();
      }
    }
  }
  fn is_closed(&self) -> bool { self.observer.rc_deref().is_none() }
}
impl<I, E, O, H, F, T> Observer<I, E> for TakeUntilNotifierObserver<O, H, F, T>
where
  O: RcDerefMut<Target = Option<T>>,
  H: Subscription,
  F: SharedCell<bool>,
{
  fn next(&mut self, _: I) {
    let observer = { self.observer.rc_deref_mut().take() };
    if let Some(observer) = observer {
      (self.complete)(observer);
      self.upstreams.take().unsubscribe();
    }
  }
  fn error(self, _: E) { self.done.set(true); }
  fn complete(self) { self.done.set(true); }
  fn is_closed(&self) -> bool { self.observer.rc_deref().is_none() }
}
#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn terminal_ownership_ignores_notifier_after_source_completion() {
    use std::cell::Cell;

    use crate::{context::TestCtx, test_support::Manual};

    TestScheduler::init();
    let source = Manual::default();
    let first_callback = source.clone();
    let mut notifier = TestCtx::subject::<(), &'static str>();
    notifier
      .clone()
      .on_error(|_| panic!("unexpected notifier error"))
      .subscribe(move |_| first_callback.complete(0));
    let completions = Rc::new(Cell::new(0));
    let completed = completions.clone();
    TestCtx::new(source.clone())
      .take_until(notifier.clone())
      .on_error(|_| panic!("unexpected error"))
      .on_complete(move || completed.set(completed.get() + 1))
      .subscribe(|_| {});

    notifier.next(());
    assert_eq!(completions.get(), 1);
    assert_eq!(source.cancellations(0), 0);
    assert_eq!(notifier.inner().subscriber_count(), 2);
    TestScheduler::flush();
    assert_eq!(notifier.inner().subscriber_count(), 1);
  }

  #[rxrust_macro::test]
  fn terminal_ownership_releases_borrow_before_source_callback() {
    use std::cell::Cell;

    use crate::test_support::Manual;

    let source = Manual::default();
    let notifier = Manual::default();
    let during_complete = notifier.clone();
    let completions = Rc::new(Cell::new(0));
    let completed = completions.clone();
    Local::new(source.clone())
      .take_until(Local::new(notifier.clone()))
      .on_error(|_| panic!("unexpected error"))
      .on_complete(move || {
        completed.set(completed.get() + 1);
        during_complete.next(0, 1);
      })
      .subscribe(|_| {});

    source.complete(0);
    assert_eq!(completions.get(), 1);
    assert_eq!(source.cancellations(0), 0);
    assert_eq!(notifier.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn notifier_finishes_once_and_cancels_in_order() {
    use crate::test_support::Manual;

    let source = Manual::default();
    let notifier = Manual::default();
    let events = Rc::new(RefCell::new(Vec::new()));
    let source_events = events.clone();
    let notifier_events = events.clone();
    let completion_events = events.clone();
    Local::new(source.clone())
      .finalize(move || source_events.borrow_mut().push("source"))
      .take_until(
        Local::new(notifier.clone())
          .finalize(move || notifier_events.borrow_mut().push("notifier")),
      )
      .on_error(|_| panic!("unexpected error"))
      .on_complete(move || completion_events.borrow_mut().push("complete"))
      .subscribe(|_| {});

    notifier.next(0, 1);
    notifier.next(0, 2);
    source.complete(0);

    assert_eq!(*events.borrow(), ["complete", "source", "notifier"]);
    assert_eq!(source.cancellations(0), 1);
    assert_eq!(notifier.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn late_value_drop_can_reenter_notifier() {
    use crate::test_support::Manual;

    struct NotifyOnDrop {
      notifier: Manual,
    }

    impl Drop for NotifyOnDrop {
      fn drop(&mut self) { self.notifier.next(0, 1); }
    }

    let source = Manual::default();
    let notifier = Manual::default();
    let during_drop = notifier.clone();
    Local::new(source.clone())
      .map(move |_| NotifyOnDrop { notifier: during_drop.clone() })
      .take_until(Local::new(notifier.clone()))
      .on_error(|_| panic!("unexpected error"))
      .subscribe(|_| panic!("unexpected value"));

    notifier.next(0, 1);
    source.next(0, 1);

    assert_eq!(source.cancellations(0), 1);
    assert_eq!(notifier.cancellations(0), 1);
  }

  #[rxrust_macro::test]
  fn naturally_terminated_notifier_is_not_cancelled() {
    use crate::test_support::Manual;

    for terminate_notifier in [Manual::complete, Manual::error] {
      let source = Manual::default();
      let notifier = Manual::default();
      let values = Rc::new(RefCell::new(Vec::new()));
      let received = values.clone();
      Local::new(source.clone())
        .take_until(Local::new(notifier.clone()))
        .on_error(|_| panic!("unexpected error"))
        .subscribe(move |value| received.borrow_mut().push(value));

      terminate_notifier(&notifier, 0);
      source.next(0, 1);
      source.complete(0);

      assert_eq!(*values.borrow(), [1]);
      assert_eq!(source.cancellations(0), 0);
      assert_eq!(notifier.cancellations(0), 0);
    }
  }

  #[rxrust_macro::test]
  fn synchronous_notifier_cancels_itself_without_starting_source() {
    use crate::subscription::ClosureSubscription;
    let calls = Rc::new(std::cell::Cell::new(0));
    let c = calls.clone();
    let notifier = Local::create::<(), Infallible, _, _>(move |e| {
      e.next(());
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    Local::create::<i32, Infallible, _, ()>(|_| panic!("source started"))
      .take_until(notifier)
      .subscribe(|_| {});
    assert_eq!(calls.get(), 1);
  }
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_take_until_emits_until_notifier_emits() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    source.next(1);
    source.next(2);
    notifier.next(());
    source.next(3);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let mut notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    notifier.next(());

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_source_complete() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(|_: i32| {});

    source.next(1);
    source.complete();

    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_take_until_notifier_complete_does_nothing() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let notifier = Local::subject::<(), Infallible>();
    let mut source = Local::subject::<i32, Infallible>();

    source
      .clone()
      .take_until(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.next(1);
    notifier.complete(); // Should be ignored
    source.next(2);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
