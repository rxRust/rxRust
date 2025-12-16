//! Merge operator implementation
//!
//! This module contains the Merge operator, which combines two observable
//! streams by subscribing to both and emitting values from either source. It
//! demonstrates multiple subscription management and completion state tracking.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::TupleSubscription,
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

#[derive(Clone)]
pub struct MergeObserver<P>(P);

pub struct MergeObserverInner<O> {
  observer: Option<O>,
  completed_one: bool,
}

impl<O, P, Item, Err> Observer<Item, Err> for MergeObserver<P>
where
  P: RcDerefMut<Target = MergeObserverInner<O>>,
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    let mut inner = self.0.rc_deref_mut();
    if let Some(ref mut observer) = inner.observer {
      observer.next(value);
    }
  }

  fn error(self, err: Err) {
    let mut inner = self.0.rc_deref_mut();
    if let Some(observer) = inner.observer.take() {
      observer.error(err);
    }
  }

  fn complete(self) {
    let mut inner = self.0.rc_deref_mut();
    if !inner.completed_one {
      inner.completed_one = true;
    } else if let Some(o) = inner.observer.take() {
      o.complete()
    }
  }

  fn is_closed(&self) -> bool {
    // If we can't inspect (e.g. inner is borrowed), we assume it's open (false)
    // or rely on recursive calls not happening in a way that blocks this.
    // For standard MutRc/MutArc, rc_deref() blocks if mutably borrowed.
    // But is_closed is called by upstream before emission, so it should be fine.
    self
      .0
      .rc_deref()
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

type MergeObserverCtx<C> = <C as Context>::With<
  MergeObserver<<C as Context>::RcMut<MergeObserverInner<<C as Context>::Inner>>>,
>;

impl<S1, S2, C> CoreObservable<C> for Merge<S1, S2>
where
  C: Context,
  S1: CoreObservable<MergeObserverCtx<C>>,
  S2: CoreObservable<MergeObserverCtx<C>>,
{
  type Unsub = TupleSubscription<S1::Unsub, S2::Unsub>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let Merge { source1, source2 } = self;
    let ctx = context.transform(|o| {
      let inner = MergeObserverInner { observer: Some(o), completed_one: false };
      MergeObserver(C::RcMut::from(inner))
    });
    let ctx2 = C::lift(ctx.inner().clone());

    let unsub1 = source1.subscribe(ctx);
    let unsub2 = source2.subscribe(ctx2);
    TupleSubscription::new(unsub1, unsub2)
  }
}

#[cfg(test)]
mod tests {
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
