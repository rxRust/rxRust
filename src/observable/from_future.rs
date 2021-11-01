use crate::prelude::*;
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
pub fn from_future<F, Item, S>(
  f: F,
  scheduler: S,
) -> ObservableBase<FutureEmitter<F, S>>
where
  F: Future<Output = Item>,
{
  ObservableBase::new(FutureEmitter {
    future: f,
    scheduler,
  })
}

#[derive(Clone)]
pub struct FutureEmitter<F, S> {
  future: F,
  scheduler: S,
}

impl<Item, F, S> Emitter for FutureEmitter<F, S>
where
  F: Future<Output = Item>,
{
  type Item = Item;
  type Err = ();
}

impl<Item, F, S> SharedEmitter for FutureEmitter<F, S>
where
  F: Future<Output = Item> + Send + Sync + 'static,
  S: SharedScheduler,
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    let f = self
      .future
      .map(move |v| SharedEmitter::emit(of::OfEmitter(v), observer));
    let (future, handle) = futures::future::abortable(f);
    self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
}

impl<Item, F, S> LocalEmitter<'static> for FutureEmitter<F, S>
where
  F: Future<Output = Item> + 'static,
  S: LocalScheduler,
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    let f = self
      .future
      .map(move |v| LocalEmitter::emit(of::OfEmitter(v), observer));
    let (future, handle) = futures::future::abortable(f);
    self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
}

/// Converts a `Future` to an observable sequence like
/// [`from_future@from_future`]. But only work for which `Future::Output` is
/// `Result` type, and `Result::Ok` emit to next handle, and `Result::Err` as an
/// error to handle.
pub fn from_future_result<F, S, Item, Err>(
  future: F,
  scheduler: S,
) -> ObservableBase<FutureResultEmitter<F, S, Item, Err>>
where
  F: Future,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  ObservableBase::new(FutureResultEmitter {
    future,
    scheduler,
    _marker: TypeHint::new(),
  })
}

#[derive(Clone)]
pub struct FutureResultEmitter<F, S, Item, Err> {
  future: F,
  scheduler: S,
  _marker: TypeHint<(Item, Err)>,
}

impl<Item, S, Err, F> Emitter for FutureResultEmitter<F, S, Item, Err>
where
  F: Future,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  type Item = Item;
  type Err = Err;
}

impl<Item, Err, S, F> SharedEmitter for FutureResultEmitter<F, S, Item, Err>
where
  Item: Send + Sync + 'static,
  Err: Send + Sync + 'static,
  F: Future + Send + Clone + Sync + 'static,
  <F as Future>::Output: Into<Result<Item, Err>>,
  S: SharedScheduler,
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    let f = self
      .future
      .map(move |v| SharedEmitter::emit(of::ResultEmitter(v.into()), observer));
    let (future, handle) = futures::future::abortable(f);
    self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
}

impl<Item, Err, S, F> LocalEmitter<'static>
  for FutureResultEmitter<F, S, Item, Err>
where
  F: Future + 'static,
  <F as Future>::Output: Into<Result<Item, Err>>,
  S: LocalScheduler,
{
  type Unsub = SpawnHandle;
  fn emit<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    let f = self
      .future
      .map(move |v| LocalEmitter::emit(of::ResultEmitter(v.into()), observer));
    let (future, handle) = futures::future::abortable(f);
    self.scheduler.spawn(future.map(|_| ()));
    SpawnHandle::new(handle)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use bencher::Bencher;
  use futures::{
    executor::{LocalPool, ThreadPool},
    future,
  };
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
  };

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
