//! Merge operator implementation
//!
//! This module contains the Merge operator, which combines two observable
//! streams by subscribing to both and emitting values from either source. It
//! demonstrates multiple subscription management and completion state tracking.

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{SingleAssignment, Subscription, TupleSubscription, single_assignment::State},
};

/// Merge operator: Combines two observable streams
///
/// This operator subscribes to both source observables and emits values from
/// either source as they arrive. The merged stream only completes when BOTH
/// source streams complete. An error from either source immediately terminates
/// the merged stream.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// let obs1 = Local::from_iter([1, 3, 5]);
/// let obs2 = Local::from_iter([2, 4, 6]);
/// let merged = obs1.merge(obs2);
/// ```
#[derive(Clone)]
pub struct Merge<S1, S2> {
  pub source1: S1,
  pub source2: S2,
}

impl<S1: ObservableType, S2> ObservableType for Merge<S1, S2> {
  type Item<'a>
    = S1::Item<'a>
  where
    Self: 'a;
  type Err = S1::Err;
}

pub struct MergeObserver<P, H> {
  state: P,
  peer: H,
}
pub struct MergeObserverInner<O> {
  observer: Option<O>,
  has_terminated_input: bool,
}
impl<O, P, H, I, E> Observer<I, E> for MergeObserver<P, H>
where
  P: RcDerefMut<Target = MergeObserverInner<O>>,
  H: Subscription,
  O: Observer<I, E>,
{
  fn next(&mut self, v: I) {
    if let Some(o) = self.state.rc_deref_mut().observer.as_mut() {
      o.next(v);
    }
  }
  fn error(self, e: E) {
    let (observer, peer_active) = {
      let mut state = self.state.rc_deref_mut();
      let peer_active = !state.has_terminated_input;
      state.has_terminated_input = true;
      (state.observer.take(), peer_active)
    };
    if let Some(o) = observer {
      o.error(e);
    }
    if peer_active {
      self.peer.unsubscribe();
    }
  }
  fn complete(self) {
    let observer = {
      let mut state = self.state.rc_deref_mut();
      if state.has_terminated_input {
        state.observer.take()
      } else {
        state.has_terminated_input = true;
        None
      }
    };
    if let Some(o) = observer {
      o.complete();
    }
  }
  fn is_closed(&self) -> bool { self.state.rc_deref().observer.is_closed() }
}
type Handle<C, U> = SingleAssignment<<C as Context>::RcMut<State<U>>>;
type Down<C> = <C as Context>::RcMut<MergeObserverInner<<C as Context>::Inner>>;
impl<S1, S2, C, U, V> CoreObservable<C> for Merge<S1, S2>
where
  C: Context,
  U: Subscription,
  V: Subscription,
  S1: CoreObservable<C::With<MergeObserver<Down<C>, ()>>, Unsub = U>
    + CoreObservable<C::With<MergeObserver<Down<C>, Handle<C, V>>>, Unsub = U>,
  S2: CoreObservable<C::With<MergeObserver<Down<C>, Handle<C, U>>>, Unsub = V>,
{
  type Unsub = TupleSubscription<Handle<C, U>, Handle<C, V>>;
  fn subscribe(self, context: C) -> Self::Unsub {
    let (observer, scheduler) = context.into_parts();
    let state =
      C::RcMut::from(MergeObserverInner { observer: Some(observer), has_terminated_input: false });
    let (install_a, [a, a_subscription]) = Handle::<C, U>::channel();
    let (install_b, [b, b_subscription]) = Handle::<C, V>::channel();
    install_a(self.source1.subscribe(C::With::from_parts(
      MergeObserver { state: state.clone(), peer: b },
      scheduler.clone(),
    )));
    if state.rc_deref().observer.is_some() {
      install_b(
        self
          .source2
          .subscribe(C::With::from_parts(MergeObserver { state, peer: a }, scheduler)),
      );
    }
    TupleSubscription::new(a_subscription, b_subscription)
  }
}

#[cfg(test)]
mod tests {

  #[rxrust_macro::test]
  fn terminal_notifications_do_not_cancel_already_terminated_inputs() {
    use std::cell::Cell;

    use crate::test_support::Manual;

    for first_input in [0, 1] {
      for first_errors in [false, true] {
        let sources = [Manual::default(), Manual::default()];
        let errors = Rc::new(Cell::new(0));
        let reported = errors.clone();
        Local::new(sources[0].clone())
          .merge(Local::new(sources[1].clone()))
          .on_error(move |_| reported.set(reported.get() + 1))
          .on_complete(|| panic!("merge completed after an error"))
          .subscribe(|_| {});

        if first_errors {
          sources[first_input].error(0);
        } else {
          sources[first_input].complete(0);
        }
        let peer = 1 - first_input;
        assert_eq!(errors.get(), usize::from(first_errors));
        assert_eq!(sources[peer].cancellations(0), usize::from(first_errors));

        sources[peer].error(0);
        assert_eq!(errors.get(), 1);
        assert_eq!(sources[first_input].cancellations(0), 0);
        assert_eq!(sources[peer].cancellations(0), usize::from(first_errors));
      }
    }
  }

  #[rxrust_macro::test(local)]
  async fn waits_for_both_and_error_cancels_only_active_peer() {
    let a = Local::subject::<i32, &str>();
    let b = Local::subject::<i32, &str>();
    let completed = Rc::new(std::cell::Cell::new(false));
    let c = completed.clone();
    a.clone()
      .merge(b.clone())
      .on_error(|_| {})
      .on_complete(move || c.set(true))
      .subscribe(|_| {});
    a.complete();
    assert!(!completed.get());
    b.complete();
    assert!(completed.get());
    let a = Local::subject::<i32, &str>();
    let b = Local::subject::<i32, &str>();
    a.clone()
      .merge(b.clone())
      .on_error(|_| {})
      .subscribe(|_| {});
    a.error("failed");
    assert_eq!(b.inner().subscriber_count(), 0);
  }
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test(local)]
  async fn test_merge_basic() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::of(1);
    let obs2 = Local::of(2);

    obs1.merge(obs2).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    // Should contain values from both sources
    let merged_result = result.borrow();
    assert_eq!(merged_result.len(), 2);
    assert!(merged_result.contains(&1));
    assert!(merged_result.contains(&2));
  }

  // #[test]
  // fn test_merge_completion() {
  //   let completed = Arc::new(AtomicBool::new(false));
  //   let completed_clone = completed.clone();

  //   let obs1 = Local::of(1);
  //   let obs2 = Local::of(2);

  //   let merged = obs1
  //     .merge(obs2)
  //     .on_complete(move || completed.store(true, Ordering::Relaxed));

  //   merged.subscribe(|_| {});

  //   // Should complete since both sources are single values and complete
  //   assert!(completed.load(Ordering::Relaxed));
  // }

  #[rxrust_macro::test(local)]
  async fn test_merge_basic_functionality() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::of(1);
    let obs2 = Local::of(2);

    obs1.merge(obs2).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    let merged_result = result.borrow();
    assert_eq!(merged_result.len(), 2);
    assert!(merged_result.contains(&1));
    assert!(merged_result.contains(&2));
  }

  #[rxrust_macro::test(local)]
  async fn test_merge_with_simple_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::of(10);
    let obs2 = Local::of(20);

    obs1.merge(obs2).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    let merged_result = result.borrow();
    assert_eq!(merged_result.len(), 2);
    assert!(merged_result.contains(&10));
    assert!(merged_result.contains(&20));
  }

  #[rxrust_macro::test(local)]
  async fn test_merge_with_string_values() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::of("hello");
    let obs2 = Local::of("world");

    obs1.merge(obs2).subscribe(move |v| {
      result_clone.borrow_mut().push(v.to_string());
    });

    let merged_result = result.borrow();
    assert_eq!(merged_result.len(), 2);
    assert!(merged_result.contains(&"hello".to_string()));
    assert!(merged_result.contains(&"world".to_string()));
  }
}
