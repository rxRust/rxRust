use crate::prelude::*;
use std::iter::{Repeat, Take};

/// Creates an observable that produces values from an iterator.
///
/// Completes when all elements have been emitted. Never emits an error.
///
/// # Arguments
///
/// * `iter` - An iterator to get all the values from.
///
/// # Examples
///
/// A simple example for a range:
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::from_iter(0..10)
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
/// Or with a vector:
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::from_iter(vec![0,1,2,3])
///   .subscribe(|v| {println!("{},", v)});
/// ```
pub fn from_iter<Iter, Item>(iter: Iter) -> ObservableBase<IterEmitter<Iter>>
where
  Iter: IntoIterator<Item = Item>,
{
  ObservableBase::new(IterEmitter(iter))
}

#[derive(Clone)]
pub struct IterEmitter<Iter>(Iter);

macro iter_emitter($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn emit<O>(self, mut subscriber: Subscriber<O, $subscription>)
  where
    O: Observer<Self::Item, Self::Err> + $($marker +)* $lf
  {
    for v in self.0.into_iter() {
      if !subscriber.is_closed() {
        subscriber.next(v);
      } else {
        break;
      }
    }
    if !subscriber.is_closed() {
      subscriber.complete();
    }
  }
}

impl<Iter, Item> Emitter for IterEmitter<Iter>
where
  Iter: IntoIterator<Item = Item>,
{
  type Item = Item;
  type Err = ();
}

impl<'a, Iter, Item> LocalEmitter<'a> for IterEmitter<Iter>
where
  Iter: IntoIterator<Item = Item>,
{
  iter_emitter!(LocalSubscription, 'a);
}

impl<Iter, Item> SharedEmitter for IterEmitter<Iter>
where
  Iter: IntoIterator<Item = Item>,
{
  iter_emitter!(SharedSubscription, Send + Sync + 'static);
}

/// Creates an observable producing same value repeated N times.
///
/// Completes immediately after emitting N values. Never emits an error.
///
/// # Arguments
///
/// * `v` - A value to emits.
/// * `n` - A number of time to repeat it.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::repeat(123, 3)
///   .subscribe(|v| {println!("{},", v)});
///
/// // print log:
/// // 123
/// // 123
/// // 123
/// ```
pub fn repeat<Item>(
  v: Item,
  n: usize,
) -> ObservableBase<IterEmitter<Take<Repeat<Item>>>>
where
  Item: Clone,
{
  from_iter(std::iter::repeat(v).take(n))
}

#[test]
fn from_range() {
  let mut hit_count = 0;
  let mut completed = false;
  observable::from_iter(0..100)
    .subscribe_complete(|_| hit_count += 1, || completed = true);

  assert_eq!(hit_count, 100);
  assert_eq!(completed, true);
}

#[test]
fn from_vec() {
  let mut hit_count = 0;
  let mut completed = false;
  observable::from_iter(vec![0; 100])
    .subscribe_complete(|_| hit_count += 1, || completed = true);

  assert_eq!(hit_count, 100);
  assert_eq!(completed, true);
}

#[test]
fn repeat_three_times() {
  let mut hit_count = 0;
  let mut completed = false;
  repeat(123, 5).subscribe_complete(
    |v| {
      hit_count += 1;
      assert_eq!(123, v);
    },
    || completed = true,
  );
  assert_eq!(5, hit_count);
  assert!(completed);
}

#[test]
fn repeat_zero_times() {
  let mut hit_count = 0;
  let mut completed = false;
  repeat(123, 0).subscribe_complete(
    |v| {
      hit_count += 1;
      assert_eq!(123, v);
    },
    || completed = true,
  );
  assert_eq!(0, hit_count);
  assert!(completed);
}
