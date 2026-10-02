//! SequenceEqual operator implementation
//!
//! Compares two observables item by item and emits one `bool`.

use std::collections::VecDeque;

use crate::{
  context::{Context, RcDeref, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::{IntoBoxedSubscription, Subscription, TupleSubscription},
};

/// SequenceEqual operator: Emits whether two sources emit equal sequences
///
/// Items are compared pairwise with `PartialEq`. Emits `false` and completes
/// at the first mismatch, or when one side completes while the other still
/// has unmatched items. Emits `true` when both complete with every item
/// matched; two empty sources are equal.
///
/// # Examples
///
/// ```rust
/// use rxrust::prelude::*;
///
/// let mut result = None;
/// Local::from_iter(vec![1, 2, 3])
///   .sequence_equal(Local::from_iter(vec![1, 2, 3]))
///   .subscribe(|v| result = Some(v));
/// assert_eq!(result, Some(true));
/// ```
#[doc(alias = "sequenceEqual")]
#[derive(Clone)]
pub struct SequenceEqual<A, B> {
  pub source_a: A,
  pub source_b: B,
}

impl<A, B> ObservableType for SequenceEqual<A, B>
where
  A: ObservableType,
{
  type Item<'a>
    = bool
  where
    Self: 'a;
  type Err = A::Err;
}

/// State shared by both sides
pub struct SequenceEqualState<O, Item> {
  observer: Option<O>,
  buffer_a: VecDeque<Item>,
  buffer_b: VecDeque<Item>,
  done_a: bool,
  done_b: bool,
}

impl<O, Item: PartialEq> SequenceEqualState<O, Item> {
  /// Compare as far as both buffers allow; returns `Some(false)` on a
  /// mismatch, `Some(true)` when both sides are done and drained, else
  /// `None`.
  fn verdict(&mut self) -> Option<bool> {
    while let (Some(a), Some(b)) = (self.buffer_a.front(), self.buffer_b.front()) {
      if a != b {
        return Some(false);
      }
      self.buffer_a.pop_front();
      self.buffer_b.pop_front();
    }
    match (self.done_a, self.done_b) {
      (true, true) => Some(self.buffer_a.is_empty() && self.buffer_b.is_empty()),
      (true, false) if self.buffer_a.is_empty() && !self.buffer_b.is_empty() => Some(false),
      (false, true) if self.buffer_b.is_empty() && !self.buffer_a.is_empty() => Some(false),
      _ => None,
    }
  }
}

/// Which side an observer feeds
#[derive(Clone, Copy)]
pub enum Side {
  /// The receiver of `sequence_equal`
  A,
  /// The argument of `sequence_equal`
  B,
}

/// Observer for one side
pub struct SequenceEqualObserver<StateRc, OtherProxy> {
  state: StateRc,
  other: OtherProxy,
  side: Side,
}

impl<StateRc, OtherProxy, O, Item> SequenceEqualObserver<StateRc, OtherProxy>
where
  StateRc: RcDerefMut<Target = SequenceEqualState<O, Item>>,
  OtherProxy: Subscription + Clone,
  Item: PartialEq,
{
  fn settle<Err>(&self)
  where
    O: Observer<bool, Err>,
  {
    let verdict = self.state.rc_deref_mut().verdict();
    if let Some(equal) = verdict {
      let observer = self.state.rc_deref_mut().observer.take();
      if let Some(mut observer) = observer {
        observer.next(equal);
        observer.complete();
      }
      // Cancel the other side; our own side stops through `is_closed`.
      self.other.clone().unsubscribe();
    }
  }
}

impl<Item, Err, O, StateRc, OtherProxy> Observer<Item, Err>
  for SequenceEqualObserver<StateRc, OtherProxy>
where
  Item: PartialEq,
  O: Observer<bool, Err>,
  StateRc: RcDerefMut<Target = SequenceEqualState<O, Item>>,
  OtherProxy: Subscription + Clone,
{
  fn next(&mut self, value: Item) {
    {
      let mut state = self.state.rc_deref_mut();
      if state.observer.is_none() {
        return;
      }
      match self.side {
        Side::A => state.buffer_a.push_back(value),
        Side::B => state.buffer_b.push_back(value),
      }
    }
    self.settle::<Err>();
  }

  fn error(self, err: Err) {
    let observer = self.state.rc_deref_mut().observer.take();
    if let Some(observer) = observer {
      observer.error(err);
    }
    self.other.unsubscribe();
  }

  fn complete(self) {
    {
      let mut state = self.state.rc_deref_mut();
      if state.observer.is_none() {
        return;
      }
      match self.side {
        Side::A => state.done_a = true,
        Side::B => state.done_b = true,
      }
    }
    self.settle::<Err>();
  }

  fn is_closed(&self) -> bool {
    self
      .state
      .rc_deref()
      .observer
      .as_ref()
      .is_none_or(|o| o.is_closed())
  }
}

type StateRc<'a, C, A> =
  <C as Context>::RcMut<SequenceEqualState<<C as Context>::Inner, <A as ObservableType>::Item<'a>>>;
type BoxedProxy<C> = <C as Context>::RcMut<Option<<C as Context>::BoxedSubscription>>;
type Proxy<C, U> = <C as Context>::RcMut<Option<U>>;

impl<A, B, C, BUnsub> CoreObservable<C> for SequenceEqual<A, B>
where
  C: Context,
  A: ObservableType
    + for<'a> CoreObservable<
      C::With<SequenceEqualObserver<StateRc<'a, C, A>, Proxy<C, BUnsub>>>,
      Unsub: IntoBoxedSubscription<C::BoxedSubscription>,
    >,
  B: for<'a> CoreObservable<
      C::With<SequenceEqualObserver<StateRc<'a, C, A>, BoxedProxy<C>>>,
      Unsub = BUnsub,
    >,
  BoxedProxy<C>: Subscription,
  Proxy<C, BUnsub>: Subscription,
{
  type Unsub = TupleSubscription<BoxedProxy<C>, Proxy<C, BUnsub>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let SequenceEqual { source_a, source_b } = self;
    let state: StateRc<C, A> = C::RcMut::from(SequenceEqualState {
      observer: Some(context.into_inner()),
      buffer_a: VecDeque::new(),
      buffer_b: VecDeque::new(),
      done_a: false,
      done_b: false,
    });
    let a_proxy: BoxedProxy<C> = C::RcMut::from(None);
    let b_proxy: Proxy<C, BUnsub> = C::RcMut::from(None);

    let a_observer =
      SequenceEqualObserver { state: state.clone(), other: b_proxy.clone(), side: Side::A };
    let a_unsub = source_a.subscribe(C::lift(a_observer));
    *a_proxy.rc_deref_mut() = Some(a_unsub.into_boxed());

    if state.rc_deref().observer.is_none() {
      // A alone decided the outcome (for example it errored); B is never
      // subscribed.
      return TupleSubscription::new(a_proxy, b_proxy);
    }

    let b_observer =
      SequenceEqualObserver { state: state.clone(), other: a_proxy.clone(), side: Side::B };
    let b_unsub = source_b.subscribe(C::lift(b_observer));
    *b_proxy.rc_deref_mut() = Some(b_unsub);

    TupleSubscription::new(a_proxy, b_proxy)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  fn run(a: Vec<i32>, b: Vec<i32>) -> Vec<bool> {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    Local::from_iter(a)
      .sequence_equal(Local::from_iter(b))
      .subscribe(move |v| result_c.borrow_mut().push(v));
    result.borrow().clone()
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_true() {
    assert_eq!(run(vec![1, 2, 3], vec![1, 2, 3]), vec![true]);
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_mismatch() {
    assert_eq!(run(vec![1, 2, 3], vec![1, 9, 3]), vec![false]);
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_length_differs() {
    assert_eq!(run(vec![1, 2], vec![1, 2, 3]), vec![false]);
    assert_eq!(run(vec![1, 2, 3], vec![1, 2]), vec![false]);
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_both_empty() {
    assert_eq!(run(vec![], vec![]), vec![true]);
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_interleaved_and_short_circuit() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();
    let mut a = Local::subject::<i32, Infallible>();
    let mut b = Local::subject::<i32, Infallible>();

    a.clone()
      .sequence_equal(b.clone())
      .subscribe(move |v| result_c.borrow_mut().push(v));

    a.next(1);
    b.next(1);
    a.next(2);
    assert!(result.borrow().is_empty());
    b.next(3);
    assert_eq!(*result.borrow(), vec![false]);
    // The other side was cancelled at the verdict
    assert_eq!(a.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_sequence_equal_error_propagation() {
    let error = Rc::new(RefCell::new(None));
    let error_c = error.clone();
    let a = Local::subject::<i32, String>();
    let b = Local::subject::<i32, String>();

    a.clone()
      .sequence_equal(b.clone())
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});
    b.error("boom".to_string());

    assert_eq!(error.borrow().as_deref(), Some("boom"));
    assert_eq!(a.inner.subscriber_count(), 0);
  }
}
