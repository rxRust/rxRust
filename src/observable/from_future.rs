use crate::{
  prelude::*,
  scheduler::{FutureTask, NormalReturn, Scheduler, TaskHandle},
};
use std::{convert::Infallible, future::Future};

/// Converts a `Future` to an observable sequence. Even though if the future
/// poll value has `Result::Err` type, also emit as a normal value, not trigger
/// to error handle.
///
/// ```rust
/// # use rxrust::prelude::*;
/// use futures::{future, executor::LocalPool};
/// let mut local_scheduler = LocalPool::new();
///
/// observable::from_future(future::ready(1), local_scheduler.spawner())
///   .subscribe(move |v| {
///     println!("subscribed {}", v);
///   });
///
/// local_scheduler.run();
/// ```
/// If your `Future` poll an `Result` type value, and you want dispatch the
/// error by rxrust, you can use [`from_future_result`]
pub fn from_future<F, Item, S>(f: F, scheduler: S) -> FutureObservable<F, S>
where
  F: Future<Output = Item>,
{
  FutureObservable { future: f, scheduler }
}

#[derive(Clone)]
pub struct FutureObservable<F, S> {
  future: F,
  scheduler: S,
}

impl<O, F, S> Observable<F::Output, Infallible, O> for FutureObservable<F, S>
where
  F: Future,
  S: Scheduler<FutureTask<F, O, NormalReturn<()>>>,
  O: Observer<F::Output, Infallible>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  #[inline]
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { future, scheduler } = self;
    scheduler.schedule(FutureTask::new(future, item_task, observer), None)
  }
}

impl<F: Future, S> ObservableExt<F::Output, Infallible>
  for FutureObservable<F, S>
{
}

fn item_task<Item, O>(item: Item, mut observer: O) -> NormalReturn<()>
where
  O: Observer<Item, Infallible>,
{
  observer.next(item);
  observer.complete();
  NormalReturn::new(())
}

/// Converts a `Future` to an observable sequence like
/// [`from_future@from_future`]. But only work for which `Future::Output` is
/// `Result` type, and `Result::Ok` emit to next handle, and `Result::Err` as an
/// error to handle.
pub fn from_future_result<F, S, Item, Err>(
  future: F,
  scheduler: S,
) -> FutureResultObservable<F, S>
where
  F: Future<Output = Result<Item, Err>>,
{
  FutureResultObservable { future, scheduler }
}

#[derive(Clone)]
pub struct FutureResultObservable<F, S> {
  future: F,
  scheduler: S,
}

impl<Item, S, Err, O, F> Observable<Item, Err, O>
  for FutureResultObservable<F, S>
where
  O: Observer<Item, Err>,
  F: Future<Output = Result<Item, Err>>,
  S: Scheduler<FutureTask<F, O, NormalReturn<()>>>,
{
  type Unsub = TaskHandle<NormalReturn<()>>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { future, scheduler } = self;
    scheduler.schedule(FutureTask::new(future, result_task, observer), None)
  }
}

fn result_task<Item, Err, O>(
  value: Result<Item, Err>,
  mut observer: O,
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  match value {
    Ok(v) => {
      observer.next(v);
      observer.complete();
    }
    Err(err) => {
      observer.error(err);
    }
  }
  NormalReturn::new(())
}

impl<Item, S, Err, F> ObservableExt<Item, Err> for FutureResultObservable<F, S> where
  F: Future<Output = Result<Item, Err>>
{
}

#[cfg(test)]
mod tests {
  use super::*;
  use bencher::Bencher;
  use futures::{executor::LocalPool, future};
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let value = Rc::new(RefCell::new(0));
    let v_c = value.clone();
    from_future_result(future::ok(1), local.spawner()).subscribe(move |v| {
      *v_c.borrow_mut() = v;
    });
    local.run();
    assert_eq!(*value.borrow(), 1);

    let v_c = value.clone();
    from_future(future::ready(2), local.spawner()).subscribe(move |v| {
      *v_c.borrow_mut() = v;
    });

    local.run();
    assert_eq!(*value.borrow(), 2);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_from_future);

  fn bench_from_future(b: &mut Bencher) {
    b.iter(local);
  }
}
