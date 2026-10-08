//! Flattens inner observables with bounded concurrency and explicit
//! cancellation.
//!
//! The shared state is split by locking rule:
//! - the downstream observer is locked while it receives events;
//! - `MergeAllControl` tracks the running upstreams, the queue and whether the
//!   operator is closed. It is never held while calling user code, so
//!   unsubscribing from inside a callback is safe.
//!
//! Every upstream (the outer source and each inner) gets a `SingleAssignment`
//! slot before it is subscribed. A cancelled slot unsubscribes whatever is
//! installed into it later, so cancelling cannot miss an upstream that is
//! still subscribing.
use std::collections::VecDeque;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{
    DynamicSubscriptions, IntoBoxedSubscription, SingleAssignment, Subscription,
    single_assignment::State,
  },
};

/// Flattens a Higher-Order Observable (an observable that emits observables).
///
/// Subscribes to inner observables as they arrive, up to `concurrent` limit.
/// Excess observables are queued and subscribed as running ones complete.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// // Basic usage - flatten nested observables
/// let mut result = Vec::new();
/// Local::from_iter([Local::from_iter([1, 2]), Local::from_iter([3, 4])])
///   .merge_all(usize::MAX)
///   .subscribe(|v| result.push(v));
/// assert_eq!(result, vec![1, 2, 3, 4]);
/// ```
///
/// # Concurrency Control
///
/// Use `concurrent` to limit simultaneous inner subscriptions:
/// - `usize::MAX`: Unlimited concurrency (subscribe to all immediately)
/// - `1`: Sequential processing (`concat_all` behavior)
/// - `n`: At most `n` concurrent inner subscriptions
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

// ==================== Control ====================

/// Lifecycle bookkeeping of one subscription.
///
/// - `Q`: a queued inner observable
/// - `H`: slot of the outer source
/// - `S`: slot of a running inner
#[doc(hidden)]
pub struct MergeAllControl<Q, H, S> {
  closed: bool,
  limit: usize,
  /// Inner observables waiting for a free slot.
  queue: VecDeque<Q>,
  /// `None` once the outer source has terminated.
  outer: Option<H>,
  inners: DynamicSubscriptions<S>,
}

impl<Q, H, S> MergeAllControl<Q, H, S> {
  fn new(limit: usize, outer: H) -> Self {
    Self {
      closed: false,
      limit,
      queue: VecDeque::new(),
      outer: Some(outer),
      inners: DynamicSubscriptions::new(),
    }
  }

  /// Nothing is running or waiting, and nothing more can arrive.
  fn is_drained(&self) -> bool {
    self.outer.is_none() && self.inners.is_empty() && self.queue.is_empty()
  }

  /// Only the first call closes. It returns the outer slot and the inner
  /// slots, to be cancelled after the lock is released.
  fn close(&mut self) -> Option<(Option<H>, Vec<S>)> {
    if self.closed {
      return None;
    }
    self.closed = true;
    self.queue.clear();
    Some((self.outer.take(), self.inners.drain().collect()))
  }
}

fn close_and_cancel<C, Q, H, S>(control: &C) -> bool
where
  C: RcDerefMut<Target = MergeAllControl<Q, H, S>>,
  H: Subscription,
  S: Subscription,
{
  let closed = { control.rc_deref_mut().close() };
  let Some((outer, inners)) = closed else {
    return false;
  };
  if let Some(outer) = outer {
    outer.unsubscribe();
  }
  for inner in inners {
    inner.unsubscribe();
  }
  true
}

// ==================== Shared ====================

/// The downstream observer (`D`) and the control (`C`) with the operations
/// that observers perform on them.
#[doc(hidden)]
#[derive(Clone)]
pub struct MergeAllShared<D, C> {
  downstream: D,
  control: C,
}

impl<D, C, O, Q, H, S> MergeAllShared<D, C>
where
  D: RcDerefMut<Target = Option<O>>,
  C: RcDerefMut<Target = MergeAllControl<Q, H, S>>,
  H: Subscription,
  S: Subscription,
{
  fn is_open(&self) -> bool { !self.control.rc_deref().closed }

  /// Delivers an event to the downstream observer, holding only its lock.
  fn emit(&self, deliver: impl FnOnce(&mut O)) {
    if self.is_open()
      && let Some(observer) = self.downstream.rc_deref_mut().as_mut()
    {
      deliver(observer);
    }
  }

  fn enqueue(&self, item: Q) {
    let rejected = {
      let mut control = self.control.rc_deref_mut();
      if control.closed {
        Some(item)
      } else {
        control.queue.push_back(item);
        None
      }
    };
    drop(rejected);
  }

  /// Forgets a finished inner. Dropping its slot does not cancel it.
  fn retire(&self, id: usize) {
    let slot = { self.control.rc_deref_mut().inners.remove(id) };
    drop(slot);
  }

  /// The outer source terminated by itself, so it must not be cancelled.
  fn outer_terminated(&self) {
    let slot = { self.control.rc_deref_mut().outer.take() };
    drop(slot);
  }

  /// Cancels every upstream, then hands the observer to `finish`. Only the
  /// first terminal event or unsubscription gets through.
  fn terminate(&self, finish: impl FnOnce(O)) {
    if !close_and_cancel(&self.control) {
      return;
    }
    let observer = { self.downstream.rc_deref_mut().take() };
    if let Some(observer) = observer {
      finish(observer);
    }
  }

  fn terminate_if_drained(&self, finish: impl FnOnce(O)) {
    let drained = { self.control.rc_deref().is_drained() };
    if drained {
      self.terminate(finish);
    }
  }
}

// ==================== Subscription ====================

pub struct MergeAllSubscription<C>(C);

impl<C, Q, H, S> Subscription for MergeAllSubscription<C>
where
  C: RcDerefMut<Target = MergeAllControl<Q, H, S>>,
  H: Subscription,
  S: Subscription,
{
  fn unsubscribe(self) { close_and_cancel(&self.0); }

  fn is_closed(&self) -> bool {
    let control = self.0.rc_deref();
    control.closed
      || (control
        .outer
        .as_ref()
        .is_none_or(Subscription::is_closed)
        && control.inners.iter().all(Subscription::is_closed))
  }
}

// ==================== Observers ====================

#[doc(hidden)]
pub struct MergeAllOuterObserver<D, C> {
  shared: MergeAllShared<D, C>,
}

#[doc(hidden)]
pub struct MergeAllInnerObserver<D, C> {
  shared: MergeAllShared<D, C>,
  /// Key of this inner's slot in `MergeAllControl::inners`.
  id: usize,
  /// Starts the next queued inner; a function pointer because this impl does
  /// not carry the bounds needed to subscribe.
  advance: fn(MergeAllShared<D, C>),
}

impl<I, E, O, D, C, H, R, U> Observer<I, E> for MergeAllOuterObserver<D, C>
where
  I: Context<
    Inner: CoreObservable<I::With<MergeAllInnerObserver<D, C>>, Unsub: IntoBoxedSubscription<U>>,
  >,
  O: for<'a> Observer<<I::Inner as ObservableType>::Item<'a>, E>,
  D: RcDerefMut<Target = Option<O>> + Clone,
  C: RcDerefMut<Target = MergeAllControl<(I::Inner, I::Scheduler), H, SingleAssignment<R>>> + Clone,
  H: Subscription,
  R: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  fn next(&mut self, inner: I) {
    self.shared.enqueue(inner.into_parts());
    advance::<I, O, D, C, H, R, U>(self.shared.clone());
  }

  fn error(self, e: E) {
    self.shared.outer_terminated();
    self
      .shared
      .terminate(|observer| observer.error(e));
  }

  fn complete(self) {
    self.shared.outer_terminated();
    self
      .shared
      .terminate_if_drained(|observer| observer.complete());
  }

  fn is_closed(&self) -> bool {
    !self.shared.is_open() || self.shared.downstream.rc_deref().is_closed()
  }
}

impl<V, E, O, D, C, Q, H, S> Observer<V, E> for MergeAllInnerObserver<D, C>
where
  O: Observer<V, E>,
  D: RcDerefMut<Target = Option<O>>,
  C: RcDerefMut<Target = MergeAllControl<Q, H, S>>,
  H: Subscription,
  S: Subscription,
{
  fn next(&mut self, v: V) { self.shared.emit(|observer| observer.next(v)); }

  fn error(self, e: E) {
    self.shared.retire(self.id);
    self
      .shared
      .terminate(|observer| observer.error(e));
  }

  fn complete(self) {
    self.shared.retire(self.id);
    self
      .shared
      .terminate_if_drained(|observer| observer.complete());
    (self.advance)(self.shared);
  }

  fn is_closed(&self) -> bool {
    !self.shared.is_open() || self.shared.downstream.rc_deref().is_closed()
  }
}

// ==================== Subscribing ====================

/// Starts the next queued inner if there is room.
fn advance<I, O, D, C, H, R, U>(shared: MergeAllShared<D, C>)
where
  I: Context<
    Inner: CoreObservable<I::With<MergeAllInnerObserver<D, C>>, Unsub: IntoBoxedSubscription<U>>,
  >,
  D: RcDerefMut<Target = Option<O>> + Clone,
  C: RcDerefMut<Target = MergeAllControl<(I::Inner, I::Scheduler), H, SingleAssignment<R>>> + Clone,
  H: Subscription,
  R: From<State<U>> + RcDerefMut<Target = State<U>>,
  U: Subscription,
{
  let ((core, scheduler), install, id) = {
    let mut control = shared.control.rc_deref_mut();
    if control.closed || control.inners.len() >= control.limit {
      return;
    }
    let Some(item) = control.queue.pop_front() else {
      return;
    };
    let (install, [slot]) = SingleAssignment::<R>::channel();
    let id = control.inners.add(slot);
    (item, install, id)
  };
  let observer =
    MergeAllInnerObserver { shared: shared.clone(), id, advance: advance::<I, O, D, C, H, R, U> };
  install(
    core
      .subscribe(I::With::from_parts(observer, scheduler))
      .into_boxed(),
  );
}

type OuterSlot<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type InnerSlot<C> =
  SingleAssignment<<C as Context>::RcMut<State<<C as Context>::BoxedSubscription>>>;
type Downstream<C> = <C as Context>::RcMut<Option<<C as Context>::Inner>>;
type Control<C, I, Sch, U> =
  <C as Context>::RcMut<MergeAllControl<(I, Sch), OuterSlot<C, U>, InnerSlot<C>>>;
type OuterCtx<C, I, Sch, U> =
  <C as Context>::With<MergeAllOuterObserver<Downstream<C>, Control<C, I, Sch, U>>>;

impl<S, C, I, Sch, U> CoreObservable<C> for MergeAll<S>
where
  C: Context,
  I: ObservableType,
  U: Subscription,
  S: for<'a> CoreObservable<
      OuterCtx<C, I, Sch, ()>,
      Unsub = U,
      Item<'a>: Context<Inner = I, Scheduler = Sch>,
    > + CoreObservable<OuterCtx<C, I, Sch, U>, Unsub = U>,
{
  type Unsub = MergeAllSubscription<Control<C, I, Sch, U>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    assert!(self.concurrent > 0, "merge_all concurrency must be positive");
    let (install_outer, [outer]) = OuterSlot::<C, U>::channel();
    let control: Control<C, I, Sch, U> =
      C::RcMut::from(MergeAllControl::new(self.concurrent, outer));
    let wrapped = context.transform(|observer| MergeAllOuterObserver {
      shared: MergeAllShared {
        downstream: C::RcMut::from(Some(observer)),
        control: control.clone(),
      },
    });
    install_outer(self.source.subscribe(wrapped));
    MergeAllSubscription(control)
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn close_cancels_slots_and_late_installations_once() {
    use crate::{
      rc::MutRc,
      subscription::{BoxedSubscription, ClosureSubscription, single_assignment::State},
    };

    let cancellations = Rc::new(std::cell::Cell::new(0));
    let cleanup = cancellations.clone();
    let mut control =
      super::MergeAllControl::<(), (), SingleAssignment<MutRc<State<_>>>>::new(1, ());
    let (install, [slot]) = SingleAssignment::<MutRc<State<BoxedSubscription>>>::channel();
    control.inners.add(slot);

    let (_, inners) = control.close().unwrap();
    assert!(control.close().is_none());
    assert!(control.inners.is_empty());
    inners
      .into_iter()
      .for_each(Subscription::unsubscribe);
    assert_eq!(cancellations.get(), 0);

    install(BoxedSubscription::new(ClosureSubscription(move || {
      cleanup.set(cleanup.get() + 1);
    })));
    assert_eq!(cancellations.get(), 1);
  }

  #[rxrust_macro::test]
  fn cancellation_and_inner_completion_preserve_late_handle_ordering() {
    use crate::{
      subscription::{BoxedSubscription, ClosureSubscription},
      test_support::Manual,
    };

    for cancel_before_terminal in [false, true] {
      let outer = Manual::default();
      let cancellations = Rc::new(std::cell::Cell::new(0));
      let holder = Rc::new(RefCell::new(None::<BoxedSubscription>));
      let during_setup = holder.clone();
      let during_next = holder.clone();
      let cleanup = cancellations.clone();
      let subscription = Local::new(outer.clone())
        .map(move |_| {
          let during_setup = during_setup.clone();
          let cleanup = cleanup.clone();
          Local::create::<i32, &'static str, _, _>(move |observer| {
            observer.next(1);
            observer.complete();
            if !cancel_before_terminal {
              during_setup
                .borrow_mut()
                .take()
                .unwrap()
                .unsubscribe();
            }
            ClosureSubscription(move || cleanup.set(cleanup.get() + 1))
          })
        })
        // Fix the mapped item type before flattening on stable Rust.
        .box_it()
        .merge_all(1)
        .on_error(|_| {})
        .subscribe(move |_| {
          if cancel_before_terminal {
            during_next
              .borrow_mut()
              .take()
              .unwrap()
              .unsubscribe();
          }
        });
      *holder.borrow_mut() = Some(BoxedSubscription::new(subscription));

      outer.next(0, 1);
      assert_eq!(outer.cancellations(0), 1);
      assert_eq!(cancellations.get(), usize::from(cancel_before_terminal));
    }
  }

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
    for outer_done in [false, true] {
      let outer = Manual::default();
      let first = Manual::default();
      let second = Manual::default();
      let inner_sources = [first.clone(), second.clone()];
      Local::new(outer.clone())
        .map(move |index| Local::new(inner_sources[index as usize].clone()))
        .merge_all(2)
        .on_error(|_| {})
        .subscribe(|_| {});
      outer.next(0, 0);
      outer.next(0, 1);
      if outer_done {
        outer.complete(0);
      }
      first.error(0);
      assert_eq!(first.cancellations(0), 0);
      assert_eq!(second.cancellations(0), 1);
      assert_eq!(outer.cancellations(0), usize::from(!outer_done));
    }
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
