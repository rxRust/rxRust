use crate::{Observable, Subscription};

pub trait Map<'a, T> {
  /// Creates a new stream which calls a closure on each element and uses
  /// its return as the value.
  ///
  fn map<B, F>(self, f: F) -> MapOP<Self, F>
  where
    Self: Sized,
    F: FnMut(T) -> B + 'a,
  {
    MapOP {
      source: self,
      func: f,
    }
  }
}

impl<'a, T, O> Map<'a, T> for O where O: Observable<'a> {}


pub struct MapOP<S, M> {
  source: S,
  func: M,
}


impl<'a, B, S, M> Observable<'a> for MapOP<S, M>
where
  S: Observable<'a>,
  M: FnMut(S::Item) -> B + 'a,
{
  type Item = B;

  fn subscribe<O>(self, mut observer: O) -> Subscription<'a>
  where
    O: 'a + FnMut(Self::Item),
  {
    let mut func = self.func;
    self.source.subscribe(move |v| {
      observer(func(v));
    })
  }
}


#[cfg(test)]
mod test {
  use crate::{ops::Map, Observable, Observer, Subject};

  #[test]
  fn primtive_type() {
    let mut i = 0;
    {
      let broadcast = Subject::new();
      broadcast.clone().map(|i| i * 2).subscribe(|v| i = v);
      broadcast.next(100);
    }
    assert_eq!(i, 200);
  }

  #[test]
  fn reference_lifetim_should_work() {
    let mut i = 0;
    {
      let broadcast = Subject::new();
      broadcast.clone().map(|v: &&i32| v).subscribe(|v| i = **v);
      broadcast.next(&100);
    }
    assert_eq!(i, 100);
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;
    {
      let broadcast = Subject::new();
      broadcast
        .clone()
        .map(|v: &&i32| v)
        .subscribe(|v| i = **v)
        .unsubscribe();
      broadcast.next(&100);
    }
    assert_eq!(i, 0);
  }
}
