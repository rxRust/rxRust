use crate::Observable;

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<'a, T> {
  fn map<B, F>(self, f: F) -> MapOp<Self, F>
  where
    Self: Sized,
    F: Fn(T) -> B + 'a,
  {
    MapOp {
      source: self,
      func: f,
    }
  }
}

impl<'a, T, O> Map<'a, T> for O where O: Observable<'a> {}

pub struct MapOp<S, M> {
  source: S,
  func: M,
}

impl<'a, B, S, M> Observable<'a> for MapOp<S, M>
where
  S: Observable<'a>,
  M: Fn(S::Item) -> B + 'a,
{
  type Item = B;
  type Err = S::Err;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    let func = self.func;
    self.source.subscribe(move |v| {
      next(func(v));
    })
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Map, Observable, Observer, Subject, Subscription};
  use std::cell::Cell;

  #[test]
  fn primitive_type() {
    let i = Cell::new(0);
    let subject = Subject::<'_, i32, ()>::new();
    subject.clone().map(|v| v * 2).subscribe(|v| i.set(v));
    subject.next(100);
    assert_eq!(i.get(), 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let i = Cell::new(0);
    let subject = Subject::<'_, &i32, ()>::new();
    subject.clone().map(|v: &&i32| v).subscribe(|v| i.set(**v));
    subject.next(&100);
    assert_eq!(i.get(), 100);
  }

  #[test]
  fn unsubscribe() {
    let i = Cell::new(0);
    let subject = Subject::<'_, &i32, ()>::new();
    subject
      .clone()
      .map(|v: &&i32| v)
      .subscribe(|v| i.set(**v))
      .unsubscribe();
    subject.next(&100);
    assert_eq!(i.get(), 0);
  }
}
