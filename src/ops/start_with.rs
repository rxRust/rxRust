use crate::{impl_local_shared_both, prelude::*};

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

impl_local_shared_both! {
  impl<S, B> StartWithOp<S, B>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    for val in $self.values {
      $observer.next(val);
    }

    $self.source.actual_subscribe($observer)
  }
  where
    S: @ctx::Observable<Item=B>,
    @ctx::shared_only(
      B: Clone + Into<S::Item> + Send + Sync + 'static,
      S::Item: 'static,
    )
    @ctx::local_only(
      B: Clone + Into<S::Item> + 'o,
      S::Item: Clone + 'o,
    )
}

#[cfg(test)]
mod test {
  use crate::of_sequence;
  use crate::prelude::*;
  use crate::test_scheduler::ManualScheduler;
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
    let scheduler = ManualScheduler::now();
    let values = Rc::new(RefCell::new(vec![]));
    let interval =
      observable::interval(Duration::from_millis(100), scheduler.clone());

    {
      let values = values.clone();
      interval
        .start_with(vec![0])
        .subscribe(move |v| values.borrow_mut().push(v));
    }

    scheduler.advance_and_run(Duration::from_millis(10), 1);
    assert_eq!(values.borrow().len(), 1);
    assert_eq!(values.borrow().as_ref(), vec![0]);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_start_with);

  fn bench_start_with(b: &mut bencher::Bencher) { b.iter(simple_integer); }
}
