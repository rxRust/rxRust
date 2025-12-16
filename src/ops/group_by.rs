//! GroupBy operator implementation
//!
//! This module contains the GroupBy operator, which splits a source observable
//! into multiple `GroupedObservable`s, each corresponding to a key derived from
//! the values.

use std::{collections::HashMap, hash::Hash, marker::PhantomData};

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// GroupBy operator: Splits the source observable into multiple
/// GroupedObservables
///
/// Each unique key derived from source values creates a new
/// `GroupedObservable`. Values with the same key are forwarded to the
/// corresponding group's observer.
///
/// # Type Parameters
///
/// - `S`: The source observable type
/// - `F`: The key selector function type
/// - `C`: Context type where `C::Inner` is `Subject<P>` for some pointer type
///   `P`
#[derive(Clone)]
pub struct GroupBy<S, F, C> {
  pub source: S,
  pub key_selector: F,
  pub _marker: PhantomData<C>,
}

impl<S, F, C> GroupBy<S, F, C> {
  /// Create a new GroupBy operator
  pub fn new(source: S, key_selector: F) -> Self {
    Self { source, key_selector, _marker: PhantomData }
  }
}

/// An observable that contains a key and emits values belonging to that key's
/// group
///
/// When subscribing to a `GroupedObservable`, you will receive all values
/// from the source that have the same key.
///
/// # Type Parameters
///
/// - `Key`: The type of the grouping key
/// - `Sub`: The subject type used for this group (typically `Subject<P>`)
#[derive(Clone)]
pub struct GroupedObservable<Key, Sub> {
  /// The key identifying this group
  pub key: Key,
  /// The inner subject for multicasting values
  pub subject: Sub,
}

/// Observer wrapper that manages group creation and value routing.
///
/// # Type Parameters
///
/// - `O`: The downstream observer receiving Context-wrapped GroupedObservables
/// - `F`: The key selector function type
/// - `Key`: The grouping key type
/// - `C`: Context type where `C::Inner` is `Subject<P>`
pub struct GroupByObserver<O, F, Key, C> {
  observer: O,
  key_selector: F,
  subjects: HashMap<Key, C>,
}

// ============================================================================
// Implementation for GroupedObservable
// ============================================================================

impl<Key, Sub> ObservableType for GroupedObservable<Key, Sub>
where
  Sub: ObservableType,
{
  type Item<'m>
    = <Sub as ObservableType>::Item<'m>
  where
    Self: 'm;
  type Err = <Sub as ObservableType>::Err;
}

impl<Key, Sub, C> CoreObservable<C> for GroupedObservable<Key, Sub>
where
  Sub: CoreObservable<C>,
{
  type Unsub = <Sub as CoreObservable<C>>::Unsub;
  fn subscribe(self, observer: C) -> Self::Unsub { self.subject.subscribe(observer) }
}

// ============================================================================
// Implementation for GroupBy
// ============================================================================
impl<S, F, Key, CtxMarker> ObservableType for GroupBy<S, F, CtxMarker>
where
  S: ObservableType,
  CtxMarker: Context,
  CtxMarker::Inner: Clone,
  F: for<'a> FnMut(&S::Item<'a>) -> Key,
{
  type Item<'m>
    = CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>
  where
    Self: 'm;
  type Err = <S as ObservableType>::Err;
}

impl<S, F, Key, CtxMarker, Ctx> CoreObservable<Ctx> for GroupBy<S, F, CtxMarker>
where
  Ctx: Context,
  CtxMarker: Context,
  CtxMarker::Inner: Clone,
  F: for<'a> FnMut(&S::Item<'a>) -> Key,
  S: CoreObservable<Ctx::With<GroupByObserver<Ctx::Inner, F, Key, CtxMarker>>>,
{
  type Unsub = S::Unsub;
  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let observer = observer.transform(|inner| GroupByObserver {
      observer: inner,
      key_selector: self.key_selector,
      subjects: HashMap::new(),
    });
    self.source.subscribe(observer)
  }
}

// ============================================================================
// Implementation for GroupByObserver
// ============================================================================
impl<Discr, Key, CtxMarker, Item, Err, O> Observer<Item, Err>
  for GroupByObserver<O, Discr, Key, CtxMarker>
where
  CtxMarker: Context,
  O: Observer<CtxMarker::With<GroupedObservable<Key, CtxMarker::Inner>>, Err>,
  Discr: FnMut(&Item) -> Key,
  Key: Hash + Eq + Clone,
  CtxMarker::Inner: Observer<Item, Err> + Clone + Default,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    let key = (self.key_selector)(&value);
    let ctx_subject = self
      .subjects
      .entry(key.clone())
      .or_insert_with(|| {
        let subject: CtxMarker::Inner = CtxMarker::Inner::default();
        let grouped = GroupedObservable { key, subject: subject.clone() };
        let wrapped = CtxMarker::lift(grouped);
        self.observer.next(wrapped);
        CtxMarker::new(subject)
      });
    ctx_subject.inner_mut().next(value);
  }

  #[inline]
  fn error(mut self, err: Err) {
    self.handle_completion(|ctx_subject| ctx_subject.into_inner().error(err.clone()));
    self.observer.error(err)
  }

  #[inline]
  fn complete(mut self) {
    self.handle_completion(|ctx_subject| ctx_subject.into_inner().complete());
    self.observer.complete()
  }

  #[inline]
  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<O, Discr, Key, CtxMarker> GroupByObserver<O, Discr, Key, CtxMarker> {
  fn handle_completion<F>(&mut self, mut action: F)
  where
    F: FnMut(CtxMarker),
  {
    for (_, ctx_subject) in self.subjects.drain() {
      action(ctx_subject);
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_group_by_parity() {
    let group_count = Rc::new(RefCell::new(0));
    let even_values = Rc::new(RefCell::new(Vec::new()));
    let odd_values = Rc::new(RefCell::new(Vec::new()));

    let group_count_clone = group_count.clone();
    let even_clone = even_values.clone();
    let odd_clone = odd_values.clone();

    Local::from_iter(0..10)
      .group_by(|v| *v % 2 == 0)
      .subscribe(move |group: Local<_>| {
        *group_count_clone.borrow_mut() += 1;
        // group is Context-wrapped, use .inner() to access key
        let key = group.inner().key;
        if key {
          let even = even_clone.clone();
          group.subscribe(move |v| even.borrow_mut().push(v));
        } else {
          let odd = odd_clone.clone();
          group.subscribe(move |v| odd.borrow_mut().push(v));
        }
      });

    assert_eq!(*group_count.borrow(), 2);
    assert_eq!(*even_values.borrow(), vec![0, 2, 4, 6, 8]);
    assert_eq!(*odd_values.borrow(), vec![1, 3, 5, 7, 9]);
  }

  #[rxrust_macro::test]
  fn test_group_by_multiple_values_same_key() {
    let results = Rc::new(RefCell::new(Vec::new()));
    let results_clone = results.clone();

    // Group strings by their first character
    Local::from_iter(vec!["apple", "apricot", "banana", "avocado", "blueberry"])
      .group_by(|s| s.chars().next().unwrap())
      .subscribe(move |group| {
        let r = results_clone.clone();
        let key = group.inner().key;
        group.subscribe(move |v| {
          r.borrow_mut().push((key, v));
        });
      });

    let received = results.borrow();
    assert_eq!(
      received
        .iter()
        .filter(|(k, _)| *k == 'a')
        .map(|(_, v)| *v)
        .collect::<Vec<_>>(),
      vec!["apple", "apricot", "avocado"]
    );
    assert_eq!(
      received
        .iter()
        .filter(|(k, _)| *k == 'b')
        .map(|(_, v)| *v)
        .collect::<Vec<_>>(),
      vec!["banana", "blueberry"]
    );
  }

  #[rxrust_macro::test]
  fn test_group_by_propagates_complete() {
    let completed_groups = Rc::new(RefCell::new(Vec::new()));
    let outer_completed = Rc::new(RefCell::new(false));

    let completed_clone = completed_groups.clone();
    let outer_clone = outer_completed.clone();

    Local::from_iter(vec![1, 2, 3])
      .group_by(|v| *v)
      .on_complete(move || *outer_clone.borrow_mut() = true)
      .subscribe(move |group: Local<_>| {
        let c = completed_clone.clone();
        let key = group.inner().key;
        group
          .on_complete(move || c.borrow_mut().push(key))
          .subscribe(|_| {});
      });

    assert!(*outer_completed.borrow());
    let mut completed = completed_groups.borrow().clone();
    completed.sort();
    assert_eq!(completed, vec![1, 2, 3]);
  }
}
