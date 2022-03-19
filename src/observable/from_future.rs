use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
use futures::FutureExt;
use observable::of;
use std::future::Future;

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
  FutureObservable {
    future: f,
    scheduler,
  }
}

#[derive(Clone)]
pub struct FutureObservable<F, S> {
  future: F,
  scheduler: S,
}

impl<Item, F, S> Observable for FutureObservable<F, S>
where
  F: Future<Output = Item>,
{
  type Item = Item;
  type Err = ();
}

impl_local_shared_both! {
  impl<F, Item, S> FutureObservable<F, S>;
  type Unsub = SpawnHandle;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let f = $self
      .future
      .map(move |v| $ctx::actual_subscribe(of::of(v), $observer));
    let (future, handle) = futures::future::abortable(f);
    $self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
  where
    @ctx::local_only('o: 'static,)
    F: Future<Output = Item> @ctx::shared_only(+ Send + Sync) + 'static,
    S: @ctx::Scheduler
}

/// Converts a `Future` to an observable sequence like
/// [`from_future@from_future`]. But only work for which `Future::Output` is
/// `Result` type, and `Result::Ok` emit to next handle, and `Result::Err` as an
/// error to handle.
pub fn from_future_result<F, S, Item, Err>(
  future: F,
  scheduler: S,
) -> FutureResultObservable<F, S, Item, Err>
where
  F: Future,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  FutureResultObservable {
    future,
    scheduler,
    _marker: TypeHint::new(),
  }
}

#[derive(Clone)]
pub struct FutureResultObservable<F, S, Item, Err> {
  future: F,
  scheduler: S,
  _marker: TypeHint<(Item, Err)>,
}

impl<Item, S, Err, F> Observable for FutureResultObservable<F, S, Item, Err>
where
  F: Future,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  type Item = Item;
  type Err = Err;
}

impl_local_shared_both! {
  impl<Item, Err, S, F> FutureResultObservable<F, S, Item, Err>;
  type Unsub = SpawnHandle;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let f = $self
      .future
      .map(move |v|$ctx::actual_subscribe( of::of_result(v.into()), $observer));
    let (future, handle) = futures::future::abortable(f);
    $self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
  where
    @ctx::local_only('o: 'static,)
    S: @ctx::Scheduler,
    F: Future @ctx::shared_only(+ Send + Sync) + 'static,
    <F as Future>::Output: Into<Result<Item, Err>>,
    S: @ctx::Scheduler
}

#[cfg(test)]
mod tests {
  use super::*;
  use bencher::Bencher;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  use futures::{executor::LocalPool, future};
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::{Arc, Mutex};
  use std::{cell::RefCell, rc::Rc};

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn shared() {
    let res = Arc::new(Mutex::new(0));
    let c_res = res.clone();
    let pool = ThreadPool::new().unwrap();
    {
      from_future_result(future::ok(1), pool.clone())
        .into_shared()
        .subscribe(move |v| {
          *res.lock().unwrap() = v;
        });
      std::thread::sleep(std::time::Duration::from_millis(10));
      assert_eq!(*c_res.lock().unwrap(), 1);
    }
    // from_future
    let res = c_res.clone();
    from_future(future::ready(2), pool)
      .into_shared()
      .subscribe(move |v| {
        *res.lock().unwrap() = v;
      });
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(*c_res.lock().unwrap(), 2);
  }

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
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_from_future);

  fn bench_from_future(b: &mut Bencher) { b.iter(local); }
}
