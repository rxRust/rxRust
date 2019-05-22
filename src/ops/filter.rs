use crate::{Observable, OnNextClousre};

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
  fn filter<N, NE>(self, filter: N) -> FilterOp<Self, N, NE>
  where
    Self: Sized,
    F: Fn(&T) -> bool + 'a,
  {
    FilterOp {
      source: self,
      filter: OnNextClousre::Next(filter),
    }
  }
}

impl<'a, T, O> Filter<'a, T> for O where O: Observable<'a, Item = T> {}

pub struct FilterOp<S, N, NE> {
  source: S,
  filter: OnNextClousre<N, NE>,
}

impl<'a, S, F, FE> Observable<'a> for FilterOp<S, F, FE>
where
  S: Observable<'a>,
  F: 'a + Fn(&S::Item) -> bool,
  FE: 'a + Fn(&S::Item) -> Result<bool, S::Err>,
{
  type Err = S::Err;
  type Item = S::Item;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + Fn(Self::Item),
  {
    let mut filter = self.filter;
    self.source.subscribe(
      move |v| {
        let res = filter.call_with_err(&v);
        match res {
          Ok(b) => {
            if b {
              next(v);
            }
          }
          Err(err) =>{
             // todo process
          },
        }
      }
    )
  }
}
