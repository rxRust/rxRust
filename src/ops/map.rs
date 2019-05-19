use crate::{Observable, ErrComplete};

/// Creates a new stream which calls a closure on each element and uses
/// its return as the value.
///
pub trait Map<'a, T> {
  fn map<B, F>(self, f: F) -> MapOp<Self, F>
  where
    Self: Sized,
    F: FnMut(T) -> B + 'a,
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
  M: FnMut(S::Item) -> B + 'a,
{
  type Item = B;
  type Unsubscribe = S::Unsubscribe;
  type Err=S::Err;

  fn subscribe<N, EC>(self, mut next: N, err_or_complete: EC) -> Self::Unsubscribe
  where
    N: 'a + FnMut(Self::Item),
    EC: 'a + FnMut(&ErrComplete<Self::Err>)
  {
    let mut func = self.func;
    self.source.subscribe(move |v| {
      next(func(v));
    }, err_or_complete)
  }
}

#[cfg(test)]
mod test {
  use crate::{ops::Map, Observable, Observer, Subject, Subscription, ErrComplete};

  #[test]
  fn primitive_type() {
    let mut i = 0;
    {
      let subject = Subject::new();
      subject.clone().map(|i| i * 2).subscribe(|v| i = v, |_: &ErrComplete<()>|{});
      subject.next(100);
    }
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetime_should_work() {
    let mut i = 0;
    {
      let subject = Subject::new();
      subject.clone().map(|v: &&i32| v).subscribe(|v| i = **v, |_: &ErrComplete<()>|{});
      subject.next(&100);
    }
    assert_eq!(i, 100);
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;
    {
      let subject = Subject::new();
      subject
        .clone()
        .map(|v: &&i32| v)
        .subscribe(|v| i = **v, |_: &ErrComplete<()>|{})
        .unsubscribe();
      subject.next(&100);
    }
    assert_eq!(i, 0);
  }
}
