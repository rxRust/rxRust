//! Buffer operator implementation.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    IntoBoxedSubscription, SingleAssignment, Subscription, TupleSubscription,
    single_assignment::State,
  },
};

/// Buffer operator.
///
/// Collects items from the source into a `Vec`, emitting the buffer when the
/// `notifier` emits. On completion of the source, any remaining items in the
/// buffer are emitted.
#[derive(Clone)]
pub struct Buffer<S, N> {
  pub source: S,
  pub notifier: N,
}

impl<S, N> ObservableType for Buffer<S, N>
where
  S: ObservableType,
  N: ObservableType<Err = S::Err>,
{
  type Item<'a>
    = Vec<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

/// Shared state maintained between the source and notifier observers.
pub struct BufferState<O, Item> {
  pub observer: Option<O>,
  pub buffer: Vec<Item>,
}

impl<O, Item> BufferState<O, Item> {
  fn new(observer: O) -> Self { Self { observer: Some(observer), buffer: Vec::new() } }
}

/// Observer for the source observable.
pub struct BufferSourceObserver<R, NotifierUnsub> {
  pub state: R,
  pub notifier_unsub: NotifierUnsub,
}

impl<R, O, Item, Err, NotifierUnsub> Observer<Item, Err> for BufferSourceObserver<R, NotifierUnsub>
where
  R: RcDerefMut<Target = BufferState<O, Item>>,
  O: Observer<Vec<Item>, Err>,
  NotifierUnsub: Subscription,
{
  fn next(&mut self, v: Item) { self.state.rc_deref_mut().buffer.push(v); }

  fn error(self, e: Err) {
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      self.notifier_unsub.unsubscribe();
      observer.error(e);
    }
  }

  fn complete(self) {
    let (mut observer, buffer) = {
      let mut state = self.state.rc_deref_mut();
      let Some(observer) = state.observer.take() else { return };
      (observer, std::mem::take(&mut state.buffer))
    };
    self.notifier_unsub.unsubscribe();
    observer.next(buffer);
    observer.complete();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref_mut().observer.is_none() }
}

/// Observer for the notifier observable.
pub struct BufferNotifierObserver<R, SourceUnsub> {
  pub state: R,
  pub source_unsub: SourceUnsub,
}

impl<R, O, Item, Err, NotifierItem, SourceUnsub> Observer<NotifierItem, Err>
  for BufferNotifierObserver<R, SourceUnsub>
where
  R: RcDerefMut<Target = BufferState<O, Item>>,
  O: Observer<Vec<Item>, Err>,
  SourceUnsub: Subscription,
{
  fn next(&mut self, _v: NotifierItem) {
    let mut state = self.state.rc_deref_mut();
    let buffer = std::mem::take(&mut state.buffer);
    if let Some(ref mut observer) = state.observer {
      observer.next(buffer);
    }
  }

  fn error(self, e: Err) {
    let observer = { self.state.rc_deref_mut().observer.take() };
    if let Some(observer) = observer {
      self.source_unsub.unsubscribe();
      observer.error(e);
    }
  }

  fn complete(self) {
    let (mut observer, buffer) = {
      let mut state = self.state.rc_deref_mut();
      let Some(observer) = state.observer.take() else { return };
      (observer, std::mem::take(&mut state.buffer))
    };
    self.source_unsub.unsubscribe();
    observer.next(buffer);
    observer.complete();
  }

  fn is_closed(&self) -> bool { self.state.rc_deref_mut().observer.is_none() }
}

// Define type aliases to simplify the generics
pub type SourceObserver<'a, C, S> = BufferSourceObserver<
  <C as Context>::RcMut<BufferState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>,
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>,
>;

pub type NotifyObserver<'a, C, S, SourceUnsub> = BufferNotifierObserver<
  <C as Context>::RcMut<BufferState<<C as Context>::Inner, <S as ObservableType>::Item<'a>>>,
  SingleAssignment<<C as Context>::RcMut<State<SourceUnsub>>>,
>;

impl<S, N, C, SourceUnsub, NotifierUnsub> CoreObservable<C> for Buffer<S, N>
where
  C: Context,
  N: ObservableType<Err = S::Err>,
  S: for<'a> CoreObservable<C::With<SourceObserver<'a, C, S>>, Unsub = SourceUnsub>,
  N: for<'a> CoreObservable<C::With<NotifyObserver<'a, C, S, SourceUnsub>>, Unsub = NotifierUnsub>,
  SourceUnsub: Subscription,
  NotifierUnsub: IntoBoxedSubscription<C::BoxedSubscription>,
  SingleAssignment<C::RcMut<State<SourceUnsub>>>: Subscription,
  SingleAssignment<C::RcMut<State<C::BoxedSubscription>>>: Subscription,
{
  type Unsub = TupleSubscription<
    SingleAssignment<C::RcMut<State<SourceUnsub>>>,
    SingleAssignment<C::RcMut<State<C::BoxedSubscription>>>,
  >;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Buffer { source, notifier } = self;

    let (downstream, scheduler) = context.into_parts();
    let state = C::RcMut::from(BufferState::new(downstream));

    let (install_source, [source_cancel, source_subscription]) =
      SingleAssignment::<C::RcMut<State<SourceUnsub>>>::channel();
    let (install_notifier, [notifier_cancel, notifier_subscription]) =
      SingleAssignment::<C::RcMut<State<C::BoxedSubscription>>>::channel();

    let source_observer =
      BufferSourceObserver { state: state.clone(), notifier_unsub: notifier_cancel };
    let source_sub = source.subscribe(C::With::from_parts(source_observer, scheduler.clone()));
    install_source(source_sub);

    if state.rc_deref().observer.is_some() {
      let notifier_observer =
        BufferNotifierObserver { state: state.clone(), source_unsub: source_cancel };
      let notifier_sub = notifier.subscribe(C::With::from_parts(notifier_observer, scheduler));
      install_notifier(notifier_sub.into_boxed());
    }

    TupleSubscription::new(source_subscription, notifier_subscription)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn terminal_ownership_precedes_peer_cleanup() {
    use crate::{context::MutRc, subscription::ClosureSubscription};

    for completes in [false, true] {
      let terminals = Rc::new(RefCell::new(vec![]));
      let errors = terminals.clone();
      let completions = terminals.clone();
      let values = Rc::new(RefCell::new(vec![]));
      let received = values.clone();
      let downstream = Local::subject::<Vec<i32>, &'static str>();
      downstream
        .clone()
        .on_error(move |error| errors.borrow_mut().push(error))
        .on_complete(move || completions.borrow_mut().push("complete"))
        .subscribe(move |value| received.borrow_mut().push(value));
      let state = MutRc::from(super::BufferState { observer: Some(downstream), buffer: vec![1] });
      let notifier = super::BufferNotifierObserver {
        state: state.clone(),
        source_unsub: ClosureSubscription(|| panic!("naturally terminated source cancelled")),
      };
      let source = super::BufferSourceObserver {
        state,
        notifier_unsub: ClosureSubscription(move || {
          Observer::<(), &'static str>::error(notifier, "cleanup");
        }),
      };

      if completes {
        source.complete();
        assert_eq!(*terminals.borrow(), vec!["complete"]);
        assert_eq!(*values.borrow(), vec![vec![1]]);
      } else {
        source.error("original");
        assert_eq!(*terminals.borrow(), vec!["original"]);
        assert!(values.borrow().is_empty());
      }
    }
  }

  #[rxrust_macro::test]
  fn synchronous_source_completion_does_not_start_notifier() {
    use std::convert::Infallible;
    let notifier = Local::create::<(), Infallible, _, ()>(|_| panic!("notifier started"));
    let mut values = vec![];
    Local::of(1)
      .buffer(notifier)
      .subscribe(|v| values.push(v));
    assert_eq!(values, vec![vec![1]]);
  }
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_buffer_with_notifier() {
    use std::convert::Infallible;

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let mut notifier = Local::subject::<(), Infallible>();

    source
      .clone()
      .buffer(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.next(1);
    source.next(2);
    notifier.next(());

    source.next(3);
    notifier.next(());

    source.next(4);
    source.complete();

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3], vec![4]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_emits_empty_on_notifier() {
    use std::convert::Infallible;

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut source = Local::subject::<i32, Infallible>();
    let mut notifier = Local::subject::<(), Infallible>();

    source
      .clone()
      .buffer(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    notifier.next(());
    source.next(1);
    notifier.next(());
    notifier.next(());

    assert_eq!(*result.borrow(), vec![vec![], vec![1], vec![]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_notifier_complete_emits_remaining() {
    use std::convert::Infallible;

    use crate::observer::Observer;

    struct TestObserver {
      result: Rc<RefCell<Vec<Vec<i32>>>>,
      completed: Rc<RefCell<bool>>,
    }

    impl Observer<Vec<i32>, Infallible> for TestObserver {
      fn next(&mut self, v: Vec<i32>) { self.result.borrow_mut().push(v); }
      fn error(self, _: Infallible) {}
      fn complete(self) { *self.completed.borrow_mut() = true; }
      fn is_closed(&self) -> bool { false }
    }

    let result = Rc::new(RefCell::new(Vec::new()));
    let completed = Rc::new(RefCell::new(false));

    let mut source = Local::subject::<i32, Infallible>();
    let notifier = Local::subject::<(), Infallible>();

    let observer = TestObserver { result: result.clone(), completed: completed.clone() };

    source
      .clone()
      .buffer(notifier.clone())
      .subscribe_with(observer);

    source.next(1);
    source.next(2);
    notifier.complete();

    assert_eq!(*result.borrow(), vec![vec![1, 2]]);
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test]
  fn test_buffer_source_complete_emits_empty() {
    use std::convert::Infallible;

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let source = Local::subject::<i32, Infallible>();
    let notifier = Local::subject::<(), Infallible>();

    source
      .clone()
      .buffer(notifier.clone())
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    source.complete();

    assert_eq!(*result.borrow(), vec![Vec::<i32>::new()]);
  }
}
