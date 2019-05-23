use crate::Observable;

/// Emit only those items from an Observable that pass a predicate test
/// # Example
///
/// ```
/// use rx_rs::{ops::Filter, Subject, Observable, Observer};
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// let subject = Subject::<'_, _, ()>::new();
/// let coll = Rc::new(RefCell::new(vec![]));
/// let coll_clone = coll.clone();
///
/// subject.clone().filter(|v| *v % 2 == 0).subscribe(move |v| {
///    coll_clone.borrow_mut().push(*v);
/// });

/// (0..10).into_iter().for_each(|v| {
///    subject.next(v);
/// });

/// // only even numbers received.
/// assert_eq!(coll.borrow().clone(), vec![0, 2, 4, 6, 8]);
/// ```

pub trait Filter<'a, T> {
  fn filter<F>(self, filter: F) -> FilterOp<Self, F>
  where
    Self: Sized,
    F: Fn(&T) -> bool + 'a,
  {
    FilterOp {
      source: self,
      filter,
    }
  }
}

impl<'a, T, O> Filter<'a, T> for O where O: Observable<'a, Item = T> {}

pub struct FilterOp<S, F> {
  source: S,
  filter: F,
}

impl<'a, S, F> Observable<'a> for FilterOp<S, F>
where
  S: Observable<'a>,
  F: 'a + Fn(&S::Item) -> bool,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    let filter = self.filter;
    self.source.subscribe(move |v| {
      if filter(&v) {
        next(v);
      }
    })
  }
}
