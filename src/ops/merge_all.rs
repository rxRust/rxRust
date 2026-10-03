//! Flattens inner observables with bounded concurrency and explicit
//! cancellation.
use std::collections::{BTreeSet, VecDeque};

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SingleAssignment, Subscription,
    single_assignment::State,
  },
};
#[derive(Clone)]
pub struct MergeAll<S> {
  pub source: S,
  pub concurrent: usize,
}
impl<S> ObservableType for MergeAll<S>
where
  S: ObservableType,
  for<'a> S::Item<'a>: Context<Inner: ObservableType>,
{
  type Item<'a>
    = <<S::Item<'a> as Context>::Inner as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}
pub struct MergeAllState<O, I, Sch> {
  observer: Option<O>,
  queue: VecDeque<(I, Sch)>,
  active: usize,
  limit: usize,
  outer_done: bool,
}
pub struct MergeAllInputs<U> {
  cancelled: bool,
  outer_done: bool,
  finished_pending: BTreeSet<usize>,
  inner: DynamicSubscriptions<Option<U>>,
}
impl<U> MergeAllInputs<U> {
  // Return the retired handle so its captures are dropped outside the inputs
  // lock.
  fn retire(&mut self, id: usize) -> Option<Option<U>> {
    let retired = self.inner.remove(id);
    if matches!(retired, Some(None)) {
      // A synchronous terminal arrived before subscribe returned its handle.
      self.finished_pending.insert(id);
    }
    retired
  }
}

pub struct MergeAllOuterObserver<P, D, H> {
  state: P,
  inputs: D,
  outer: H,
}
pub struct MergeAllInnerObserver<P, D, H> {
  state: P,
  inputs: D,
  outer: H,
  id: usize,
  advance: fn(P, D, H),
}
pub struct MergeAllSubscription<H, D> {
  outer: H,
  inputs: D,
}
impl<H, D, U> Subscription for MergeAllSubscription<H, D>
where
  H: Subscription,
  D: RcDerefMut<Target = MergeAllInputs<U>>,
  U: Subscription,
{
  fn unsubscribe(self) {
    let handles = {
      let mut inputs = self.inputs.rc_deref_mut();
      inputs.cancelled = true;
      inputs.inner.drain().collect::<Vec<_>>()
    };
    self.outer.unsubscribe();
    for handle in handles {
      handle.unsubscribe();
    }
  }
  fn is_closed(&self) -> bool {
    let inputs = self.inputs.rc_deref();
    inputs.cancelled
      || (self.outer.is_closed()
        && inputs
          .inner
          .iter()
          .all(|u| u.as_ref().is_some_and(Subscription::is_closed)))
  }
}
type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type Data<C, I, Sch> = <C as Context>::RcMut<MergeAllState<<C as Context>::Inner, I, Sch>>;
type Inputs<C> = <C as Context>::RcMut<MergeAllInputs<<C as Context>::BoxedSubscription>>;
type OuterCtx<C, I, Sch, H> =
  <C as Context>::With<MergeAllOuterObserver<Data<C, I, Sch>, Inputs<C>, H>>;
impl<S, C, I, Sch, U> CoreObservable<C> for MergeAll<S>
where
  C: Context,
  I: ObservableType,
  U: Subscription,
  S: for<'a> CoreObservable<
      OuterCtx<C, I, Sch, ()>,
      Unsub = U,
      Item<'a>: Context<Inner = I, Scheduler = Sch>,
    > + CoreObservable<OuterCtx<C, I, Sch, Handle<C, U>>, Unsub = U>,
{
  type Unsub = MergeAllSubscription<Handle<C, U>, Inputs<C>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    assert!(self.concurrent > 0, "merge_all concurrency must be positive");
    let outer = Handle::<C, U>::new();
    let inputs = C::RcMut::from(MergeAllInputs {
      cancelled: false,
      outer_done: false,
      finished_pending: BTreeSet::new(),
      inner: DynamicSubscriptions::new(),
    });
    let wrapped = context.transform(|observer| MergeAllOuterObserver {
      state: C::RcMut::from(MergeAllState {
        observer: Some(observer),
        queue: VecDeque::new(),
        active: 0,
        limit: self.concurrent,
        outer_done: false,
      }),
      inputs: inputs.clone(),
      outer: outer.clone(),
    });
    outer.set(self.source.subscribe(wrapped));
    MergeAllSubscription { outer, inputs }
  }
}
fn cancel_inputs<D, H, U>(inputs: &D, outer: H)
where
  D: RcDerefMut<Target = MergeAllInputs<U>>,
  H: Subscription,
  U: Subscription,
{
  let (handles, outer_done) = {
    let mut inputs = inputs.rc_deref_mut();
    inputs.cancelled = true;
    (inputs.inner.drain().collect::<Vec<_>>(), inputs.outer_done)
  };
  if !outer_done {
    outer.unsubscribe();
  }
  for handle in handles {
    handle.unsubscribe();
  }
}
impl<I, E, O, P, D, H, U> Observer<I, E> for MergeAllOuterObserver<P, D, H>
where
  I: Context<
    Inner: CoreObservable<I::With<MergeAllInnerObserver<P, D, H>>, Unsub: IntoBoxedSubscription<U>>,
  >,
  O: for<'a> Observer<<I::Inner as ObservableType>::Item<'a>, E>,
  P: RcDerefMut<Target = MergeAllState<O, I::Inner, I::Scheduler>>,
  D: RcDerefMut<Target = MergeAllInputs<U>>,
  H: Subscription + Clone,
  U: Subscription,
{
  fn next(&mut self, inner: I) {
    if self.inputs.rc_deref().cancelled {
      return;
    }
    self
      .state
      .rc_deref_mut()
      .queue
      .push_back(inner.into_parts());
    advance::<I, O, P, D, H, U>(self.state.clone(), self.inputs.clone(), self.outer.clone());
  }
  fn error(self, e: E) {
    self.inputs.rc_deref_mut().outer_done = true;
    let observer = {
      let mut state = self.state.rc_deref_mut();
      state.queue.clear();
      state.observer.take()
    };
    cancel_inputs(&self.inputs, self.outer);
    if let Some(observer) = observer {
      observer.error(e);
    }
  }
  fn complete(self) {
    self.inputs.rc_deref_mut().outer_done = true;
    let observer = {
      let mut state = self.state.rc_deref_mut();
      state.outer_done = true;
      if state.active == 0 && state.queue.is_empty() { state.observer.take() } else { None }
    };
    if let Some(observer) = observer {
      observer.complete();
    }
  }
  fn is_closed(&self) -> bool {
    self.inputs.rc_deref().cancelled || self.state.rc_deref().observer.is_closed()
  }
}
fn advance<I, O, P, D, H, U>(state: P, inputs: D, outer: H)
where
  I: Context<
    Inner: CoreObservable<I::With<MergeAllInnerObserver<P, D, H>>, Unsub: IntoBoxedSubscription<U>>,
  >,
  P: RcDerefMut<Target = MergeAllState<O, I::Inner, I::Scheduler>>,
  D: RcDerefMut<Target = MergeAllInputs<U>>,
  H: Subscription + Clone,
  U: Subscription,
{
  let next = {
    let mut state = state.rc_deref_mut();
    if inputs.rc_deref().cancelled || state.observer.is_none() || state.active >= state.limit {
      return;
    }
    let next = state.queue.pop_front();
    if next.is_some() {
      state.active += 1;
    }
    next
  };
  if let Some((core, scheduler)) = next {
    let id = {
      let mut inputs = inputs.rc_deref_mut();
      if inputs.cancelled {
        return;
      }
      inputs.inner.add(None)
    };
    let ctx = I::With::from_parts(
      MergeAllInnerObserver {
        state: state.clone(),
        inputs: inputs.clone(),
        outer,
        id,
        advance: advance::<I, O, P, D, H, U>,
      },
      scheduler,
    );
    let mut handle = Some(core.subscribe(ctx).into_boxed());
    let cancel = {
      let mut inputs = inputs.rc_deref_mut();
      if inputs.finished_pending.remove(&id) {
        false
      } else if inputs.cancelled {
        true
      } else {
        if inputs.inner.contains(id) {
          inputs.inner.remove(id);
          inputs.inner.insert(id, handle.take());
        }
        false
      }
    };
    if cancel {
      handle.unsubscribe();
    }
  }
}
impl<V, E, O, I, Sch, P, D, H, U> Observer<V, E> for MergeAllInnerObserver<P, D, H>
where
  O: Observer<V, E>,
  P: RcDerefMut<Target = MergeAllState<O, I, Sch>>,
  D: RcDerefMut<Target = MergeAllInputs<U>>,
  H: Subscription + Clone,
  U: Subscription,
{
  fn next(&mut self, v: V) {
    if self.inputs.rc_deref().cancelled {
      return;
    }
    if let Some(o) = self.state.rc_deref_mut().observer.as_mut() {
      o.next(v);
    }
  }
  fn error(self, e: E) {
    let retired = { self.inputs.rc_deref_mut().retire(self.id) };
    drop(retired);
    let observer = {
      let mut state = self.state.rc_deref_mut();
      state.queue.clear();
      state.observer.take()
    };
    cancel_inputs(&self.inputs, self.outer);
    if let Some(o) = observer {
      o.error(e);
    }
  }
  fn complete(self) {
    let retired = { self.inputs.rc_deref_mut().retire(self.id) };
    drop(retired);
    if self.inputs.rc_deref().cancelled {
      return;
    }
    let observer = {
      let mut state = self.state.rc_deref_mut();
      state.active -= 1;
      if state.outer_done && state.active == 0 && state.queue.is_empty() {
        state.observer.take()
      } else {
        None
      }
    };
    if let Some(o) = observer {
      o.complete();
    }
    (self.advance)(self.state, self.inputs, self.outer);
  }
  fn is_closed(&self) -> bool {
    self.inputs.rc_deref().cancelled || self.state.rc_deref().observer.is_closed()
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn synchronous_inner_completion_does_not_retain_or_cancel_late_handle() {
    use crate::subscription::ClosureSubscription;
    let cancelled = Rc::new(std::cell::Cell::new(0));
    let c = cancelled.clone();
    let inner = Local::create(move |e| {
      e.next(1);
      e.complete();
      ClosureSubscription(move || c.set(c.get() + 1))
    });
    let sub = Local::of(inner).merge_all(1).subscribe(|_| {});
    sub.unsubscribe();
    assert_eq!(cancelled.get(), 0);
  }
  #[rxrust_macro::test]
  fn cancellation_from_callback_never_starts_queued_inner() {
    use crate::{subscription::BoxedSubscription, test_support::Manual};
    let first = Manual::default();
    let second = Manual::default();
    let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
    let h = holder.clone();
    let sub = Local::from_iter([Local::new(first.clone()), Local::new(second.clone())])
      .map_err(|never| -> &'static str { match never {} })
      .merge_all(1)
      .on_error(|_| {})
      .subscribe(move |_| h.borrow_mut().take().unwrap().unsubscribe());
    *holder.borrow_mut() = Some(BoxedSubscription::new(sub));
    first.next(0, 1);
    first.complete(0);
    assert_eq!(second.subscriptions(), 0);
    assert_eq!(first.cancellations(0), 1);
  }
  #[rxrust_macro::test]
  fn inner_error_cancels_peers_but_not_naturally_terminated_input() {
    use crate::test_support::Manual;
    let first = Manual::default();
    let second = Manual::default();
    Local::from_iter([Local::new(first.clone()), Local::new(second.clone())])
      .map_err(|never| -> &'static str { match never {} })
      .merge_all(2)
      .on_error(|_| {})
      .subscribe(|_| {});
    first.error(0);
    assert_eq!(first.cancellations(0), 0);
    assert_eq!(second.cancellations(0), 1);
  }
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_merge_all_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // Create observable of observables - no .into_inner() needed!
    Local::from_iter([Local::from_iter([1, 2]), Local::from_iter([3, 4])])
      .merge_all(usize::MAX)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn test_concat_all() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // concat_all is merge_all(1) - sequential subscription
    Local::from_iter([Local::from_iter([1, 2]), Local::from_iter([3, 4])])
      .concat_all()
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert_eq!(*result.borrow(), vec![1, 2, 3, 4]);
  }

  #[rxrust_macro::test]
  fn test_merge_all_empty() {
    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    Local::from_iter(Vec::<Local<FromIter<std::vec::IntoIter<i32>>>>::new())
      .merge_all(usize::MAX)
      .on_complete(move || {
        *completed_clone.borrow_mut() = true;
      })
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert!(result.borrow().is_empty());
    assert!(*completed.borrow());
  }

  #[rxrust_macro::test(local)]
  async fn test_merge_all_subscription_stays_open_while_inner_active() {
    use std::time::Duration;

    use crate::{scheduler::LocalScheduler, subscription::Subscription};

    let values = Rc::new(RefCell::new(Vec::new()));
    let values_c = values.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_c = completed.clone();

    let subscription = Local::of(Local::of(42).delay_subscription(Duration::from_millis(20)))
      .merge_all(usize::MAX)
      .on_complete(move || *completed_c.borrow_mut() = true)
      .subscribe(move |v| values_c.borrow_mut().push(v));

    assert!(!subscription.is_closed());
    assert!(values.borrow().is_empty());
    assert!(!*completed.borrow());

    LocalScheduler
      .sleep(Duration::from_millis(40))
      .await;

    assert_eq!(*values.borrow(), vec![42]);
    assert!(*completed.borrow());
    assert!(subscription.is_closed());
  }

  #[rxrust_macro::test]
  fn test_merge_all_concurrency() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut s1 = Local::subject();
    let mut s2 = Local::subject();
    let mut s3 = Local::subject();

    let mut outer = Local::subject();

    let _subscription = outer.clone().merge_all(2).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    // Pass Context-wrapped subjects (Local<Subject>)
    outer.next(s1.clone());
    outer.next(s2.clone());
    outer.next(s3.clone()); // queued

    s1.next(1);
    s2.next(2);
    s3.next(3); // Should be lost since s3 is queued

    assert_eq!(*result.borrow(), vec![1, 2]);

    s1.complete(); // s3 should be subscribed now

    s3.next(4);
    assert_eq!(*result.borrow(), vec![1, 2, 4]);
  }

  #[rxrust_macro::test]
  fn test_merge_all_inner_error() {
    let error_called = Rc::new(RefCell::new(false));
    let error_called_clone = error_called.clone();

    let s1 = Local::subject();
    let mut outer = Local::subject();

    outer
      .clone()
      .merge_all(usize::MAX)
      .on_error(move |_| {
        *error_called_clone.borrow_mut() = true;
      })
      .subscribe(|_: i32| {});

    // Pass Context-wrapped subject
    outer.next(s1.clone());
    s1.error(());

    assert!(*error_called.borrow());
  }

  #[rxrust_macro::test]
  fn test_merge_all_outer_error() {
    let error_called = Rc::new(RefCell::new(false));
    let error_called_clone = error_called.clone();

    let outer = Local::subject();
    let inner_dummy = Local::subject::<i32, ()>();

    let mut outer_clone = outer.clone();

    outer
      .clone()
      .merge_all(usize::MAX)
      .on_error(move |_| {
        *error_called_clone.borrow_mut() = true;
      })
      .subscribe(|_| {});

    // Force type inference with Context-wrapped subject
    if false {
      outer_clone.next(inner_dummy.clone());
    }

    outer.error(());

    assert!(*error_called.borrow());
  }

  #[rxrust_macro::test]
  fn test_merge_all_unsubscribe() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut s1 = Local::subject();
    let mut s2 = Local::subject();
    let mut outer = Local::subject();

    let subscription = outer
      .clone()
      .merge_all(usize::MAX)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Pass Context-wrapped subjects
    outer.next(s1.clone());
    outer.next(s2.clone());

    s1.next(1);
    s2.next(2);

    assert_eq!(*result.borrow(), vec![1, 2]);

    subscription.unsubscribe();

    s1.next(3);
    s2.next(4);

    assert_eq!(*result.borrow(), vec![1, 2]);
  }
}
