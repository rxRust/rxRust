//! Flattens Higher-Order Observables by merging inner observable emissions.

use std::collections::VecDeque;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{DynamicSubscriptions, IntoBoxedSubscription},
};
/// Flattens a Higher-Order Observable (an observable that emits observables).
///
/// Subscribes to inner observables as they arrive, up to `concurrent` limit.
/// Excess observables are queued and subscribed when slots become available.
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

/// Shared state for MergeAll observers
#[doc(hidden)]
pub struct MergeAllState<O, InnerObs> {
  observer: Option<O>,
  /// Queue of pending inner observables when at concurrent limit
  pending_observables: VecDeque<InnerObs>,
  /// Number of currently active inner subscriptions
  subscribed: usize,
  /// Maximum concurrent inner subscriptions
  concurrent: usize,
  /// Whether the outer observable has completed
  outer_completed: bool,
}

impl<O, InnerObs> MergeAllState<O, InnerObs> {
  /// Checks if the observer is closed.
  /// Used by both inner and outer observers' `is_closed` implementation.
  fn observer_is_closed<Item, Err>(&self) -> bool
  where
    O: Observer<Item, Err>,
  {
    self.observer.as_ref().is_none_or(O::is_closed)
  }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct MergeAllOuterObserver<P, SubState> {
  state: P,
  sub_state: SubState,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct MergeAllInnerObserver<P, SubState> {
  state: P,
  sub_state: SubState,
  /// ID of this observer's subscription in DynamicSubscriptions
  subscription_id: usize,
  /// Function pointer to subscribe to queued inner observables
  subscribe_fn: fn(P, SubState),
}

/// Subscription handle returned by `merge_all`.
///
/// Unsubscribing cancels both the outer subscription and all active inner
/// subscriptions.
pub type MergeAllSubscription<SrcUnsub, SubState> =
  crate::subscription::SourceWithDynamicSubs<SrcUnsub, SubState>;

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

/// Type alias for subscription state (using DynamicSubscriptions)
type RcSubscriptions<C> =
  <C as Context>::RcMut<DynamicSubscriptions<<C as Context>::BoxedSubscription>>;

/// CoreObservable implementation for MergeAll
///
/// Inner observables must be Context-wrapped (like `Local<FromIter<...>>`).
/// The Context is stripped via `into_inner()` when subscribing to ensure
/// all inner subscriptions use the outer Context's execution environment.
impl<S, C, InnerObs> CoreObservable<C> for MergeAll<S>
where
  C: Context,
  S: for<'a> CoreObservable<
      C::With<
        MergeAllOuterObserver<
          C::RcMut<MergeAllState<C::Inner, InnerObs>>,
          C::RcMut<DynamicSubscriptions<C::BoxedSubscription>>,
        >,
      >,
      Item<'a>: Context<Inner = InnerObs>,
    >,
  InnerObs: ObservableType,
{
  type Unsub = MergeAllSubscription<S::Unsub, RcSubscriptions<C>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let rc_sub_state: RcSubscriptions<C> = C::RcMut::from(DynamicSubscriptions::default());

    let ctx = context.transform(|observer| {
      let state = C::RcMut::from(MergeAllState {
        observer: Some(observer),
        pending_observables: VecDeque::new(),
        subscribed: 0,
        concurrent: self.concurrent,
        outer_completed: false,
      });
      MergeAllOuterObserver { state, sub_state: rc_sub_state.clone() }
    });

    let src_unsub = self.source.subscribe(ctx);
    crate::subscription::SourceWithDynamicSubs::new(src_unsub, rc_sub_state)
  }
}

impl<Item, Err, O, InnerObs, RcState, RcUnsubs, DynUnsub> Observer<Item, Err>
  for MergeAllInnerObserver<RcState, RcUnsubs>
where
  O: Observer<Item, Err>,
  RcState: RcDerefMut<Target = MergeAllState<O, InnerObs>> + Clone,
  RcUnsubs: RcDerefMut<Target = DynamicSubscriptions<DynUnsub>> + Clone,
{
  fn next(&mut self, value: Item) {
    let mut state = self.state.rc_deref_mut();
    if let Some(ref mut observer) = state.observer {
      observer.next(value);
    }
  }

  fn error(self, err: Err) {
    let mut state = self.state.rc_deref_mut();
    if let Some(observer) = state.observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    // Remove this subscription from the collection
    self
      .sub_state
      .rc_deref_mut()
      .remove(self.subscription_id);

    let mut state = self.state.rc_deref_mut();
    // Subscribe to next queued observable if available
    if !state.pending_observables.is_empty() {
      drop(state);
      (self.subscribe_fn)(self.state.clone(), self.sub_state.clone());
    } else {
      state.subscribed = state.subscribed.saturating_sub(1);
      // Complete downstream if outer completed and no more active subscriptions
      if state.subscribed == 0
        && state.outer_completed
        && let Some(observer) = state.observer.take()
      {
        observer.complete();
      }
    }
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .observer_is_closed::<Item, Err>()
  }
}

/// Observer implementation for outer observable
///
/// Receives Context-wrapped inner observables and strips the Context via
/// `into_inner()` before storing them. This ensures all inner subscriptions
/// use the outer Context's execution environment.
impl<Ctx, Err, O, RcState, RcUnsubs> Observer<Ctx, Err> for MergeAllOuterObserver<RcState, RcUnsubs>
where
  Ctx: Context<
    Inner: CoreObservable<
      Ctx::With<MergeAllInnerObserver<RcState, RcUnsubs>>,
      Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
    >,
  >,
  O: for<'a> Observer<<Ctx::Inner as ObservableType>::Item<'a>, Err>,
  RcState: RcDerefMut<Target = MergeAllState<O, Ctx::Inner>> + Clone,
  RcUnsubs: RcDerefMut<Target = DynamicSubscriptions<Ctx::BoxedSubscription>> + Clone,
{
  fn next(&mut self, inner_ctx_obs: Ctx) {
    // Strip the Context to get the raw CoreObservable
    let inner_obs = inner_ctx_obs.into_inner();
    let mut state = self.state.rc_deref_mut();
    state.pending_observables.push_back(inner_obs);
    if state.subscribed < state.concurrent {
      state.subscribed += 1;
      drop(state);
      Self::do_next_subscribe::<Ctx, _, _>(self.state.clone(), self.sub_state.clone());
    }
  }

  fn error(self, err: Err) {
    let mut state = self.state.rc_deref_mut();
    if let Some(observer) = state.observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    let mut state = self.state.rc_deref_mut();
    state.outer_completed = true;
    // Complete downstream if no active subscriptions and no pending observables
    if state.subscribed == 0
      && state.pending_observables.is_empty()
      && let Some(observer) = state.observer.take()
    {
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref().observer_is_closed() }
}

impl<RcState, RcUnsubs> MergeAllOuterObserver<RcState, RcUnsubs> {
  fn do_next_subscribe<Ctx, O, InnerObs>(state: RcState, sub_state: RcUnsubs)
  where
    Ctx: Context,
    RcState: RcDerefMut<Target = MergeAllState<O, InnerObs>> + Clone,
    RcUnsubs: RcDerefMut<Target = DynamicSubscriptions<Ctx::BoxedSubscription>> + Clone,
    InnerObs: CoreObservable<
        Ctx::With<MergeAllInnerObserver<RcState, RcUnsubs>>,
        Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
      >,
  {
    let inner_obs = state
      .rc_deref_mut()
      .pending_observables
      .pop_front();
    if let Some(inner_obs) = inner_obs {
      // Pre-allocate ID before creating observer
      let subscription_id = sub_state.rc_deref_mut().reserve_id();

      let inner_observer = MergeAllInnerObserver {
        state: state.clone(),
        sub_state: sub_state.clone(),
        subscription_id,
        subscribe_fn: Self::do_next_subscribe::<Ctx, _, _>,
      };

      let unsub = inner_obs.subscribe(Ctx::lift(inner_observer));
      let boxed_unsub = unsub.into_boxed();

      // Insert subscription with pre-allocated ID
      sub_state
        .rc_deref_mut()
        .insert(subscription_id, boxed_unsub);
    }
  }
}

#[cfg(test)]
mod tests {
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
