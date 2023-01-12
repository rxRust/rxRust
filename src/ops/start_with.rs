use crate::prelude::*;

#[derive(Clone)]
pub struct StartWithOp<S, B> {
  pub(crate) source: S,
  pub(crate) values: Vec<B>,
}

impl<Item, Err, O, S> Observable<Item, Err, O> for StartWithOp<S, Item>
where
  S: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    for val in self.values {
      observer.next(val);
    }

    self.source.actual_subscribe(observer)
  }
}

impl<Item, Err, S> ObservableExt<Item, Err> for StartWithOp<S, Item> where
  S: ObservableExt<Item, Err>
{
}

#[cfg(test)]
mod test {
  use crate::observable::fake_timer::FakeClock;
  use crate::of_sequence;
  use crate::prelude::*;
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::time::Duration;

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
        ret.push_str(value);
      });
    }

    assert_eq!(ret, "Hello World! Goodbye World!");
  }

  #[test]
  fn should_start_on_subscription() {
    let clock = FakeClock::default();
    let values = Rc::new(RefCell::new(vec![]));
    let interval = clock.interval(Duration::from_millis(100));

    {
      let values = values.clone();
      interval
        .start_with(vec![0])
        .subscribe(move |v| values.borrow_mut().push(v));
    }

    clock.advance(Duration::from_millis(10));
    assert_eq!(values.borrow().len(), 1);
    assert_eq!(values.borrow().as_ref(), vec![0]);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_start_with);

  fn bench_start_with(b: &mut bencher::Bencher) {
    b.iter(simple_integer);
  }
}
