use crate::prelude::*;
#[derive(Clone)]
pub struct TakeWhileOp<S, F> {
  pub(crate) source: S,
  pub(crate) callback: F,
  pub(crate) inclusive: bool,
}

impl<S, F, Item, Err, O> Observable<Item, Err, O> for TakeWhileOp<S, F>
where
  O: Observer<Item, Err>,
  TakeWhileObserver<O, F>: Observer<Item, Err>,
  S: Observable<Item, Err, TakeWhileObserver<O, F>>,
  F: FnMut(&Item) -> bool,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let observer = TakeWhileObserver {
      observer: Some(observer),
      callback: self.callback,
      inclusive: self.inclusive,
    };
    self.source.actual_subscribe(observer)
  }
}

impl<S, F, Item, Err> ObservableExt<Item, Err> for TakeWhileOp<S, F> where
  S: ObservableExt<Item, Err>
{
}

pub struct TakeWhileObserver<O, F> {
  observer: Option<O>,
  callback: F,
  inclusive: bool,
}

impl<O, Item, Err, F> Observer<Item, Err> for TakeWhileObserver<O, F>
where
  O: Observer<Item, Err>,
  F: FnMut(&Item) -> bool,
{
  fn next(&mut self, value: Item) {
    if let Some(observer) = self.observer.as_mut() {
      if (self.callback)(&value) {
        observer.next(value);
      } else {
        if self.inclusive {
          observer.next(value);
        }
        self.observer.take().unwrap().complete()
      }
    }
  }

  #[inline]
  fn error(self, err: Err) {
    if let Some(o) = self.observer {
      o.error(err)
    }
  }

  #[inline]
  fn complete(self) {
    if let Some(o) = self.observer {
      o.complete()
    }
  }

  fn is_finished(&self) -> bool {
    self.observer.as_ref().map_or(true, |o| o.is_finished())
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take_while(|v| v < &5)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 5);
    assert!(completed);
  }

  #[test]
  fn inclusive_case() {
    let mut completed = false;
    let mut next_count = 0;

    observable::from_iter(0..100)
      .take_while_inclusive(|v| v < &5)
      .on_complete(|| completed = true)
      .subscribe(|_| next_count += 1);

    assert_eq!(next_count, 6);
    assert!(completed);
  }

  #[test]
  fn take_while_support_fork() {
    let mut nc1 = 0;
    let mut nc2 = 0;
    {
      let take_while5 = observable::from_iter(0..100).take_while(|v| v < &5);
      let f1 = take_while5.clone();
      let f2 = take_while5;

      f1.take_while(|v| v < &5).subscribe(|_| nc1 += 1);
      f2.take_while(|v| v < &5).subscribe(|_| nc2 += 1);
    }
    assert_eq!(nc1, 5);
    assert_eq!(nc2, 5);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_take_while);

  fn bench_take_while(b: &mut bencher::Bencher) {
    b.iter(base_function);
  }
}
