use crate::prelude::*;

#[derive(Clone)]
pub struct StartWithOp<S, B> {
  pub(crate) source: S,
  pub(crate) values: Vec<B>,
}

impl<S, B> Observable for StartWithOp<S, B>
where
  S: Observable,
{
  type Item = S::Item;
  type Err = S::Err;
}

#[doc(hidden)]
macro_rules! observable_impl {
  ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
    fn actual_subscribe<O> (
      self,
      observer: O,
    ) -> Self::Unsub
    where O: Observer<Item = Self::Item, Err = Self::Err> + $($marker +)* $lf {
      let values = self.values;

      self.source.actual_subscribe(StartWithObserver {
        observer,
        values,
        is_values_processed: false,
        _marker: TypeHint::new(),
      })
    }
  }
}

impl<'a, B, S> LocalObservable<'a> for StartWithOp<S, B>
where
  S: LocalObservable<'a>,
  B: Clone + Into<S::Item> + 'a,
  S::Item: 'a,
{
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription,'a);
}

impl<B, S> SharedObservable for StartWithOp<S, B>
where
  S: SharedObservable,
  B: Clone + Into<S::Item> + Send + Sync + 'static,
  S::Item: 'static,
{
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

#[derive(Clone)]
pub struct StartWithObserver<O, B, Item> {
  observer: O,
  values: Vec<B>,
  is_values_processed: bool,
  _marker: TypeHint<*const Item>,
}

impl<Item, Err, O, B> Observer for StartWithObserver<O, B, Item>
where
  O: Observer<Item = Item, Err = Err>,
  B: Clone + Into<Item>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    if !self.is_values_processed {
      for val in self.values.clone() {
        self.observer.next(val.into());
      }

      self.is_values_processed = true;
    }

    self.observer.next(value)
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }

  fn complete(&mut self) { self.observer.complete() }
}

#[cfg(test)]
mod test {
  use crate::of_sequence;
  use crate::prelude::*;

  #[test]
  fn simple_integer() {
    let mut ret = String::new();

    {
      let s = of_sequence!(1, 2, 3);

      s.start_with(vec![-1, 0]).subscribe(|value| {
        ret.push_str(&value.to_string());
      });
    }

    assert_eq!(ret, "-10123");
  }

  #[test]
  fn simple_string() {
    let mut ret = String::new();

    {
      let s = of_sequence!(" World!", " Goodbye", " World!");

      s.start_with(vec!["Hello"]).subscribe(|value| {
        ret.push_str(&value.to_string());
      });
    }

    assert_eq!(ret, "Hello World! Goodbye World!");
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_start_with);

  fn bench_start_with(b: &mut bencher::Bencher) { b.iter(simple_integer); }
}
