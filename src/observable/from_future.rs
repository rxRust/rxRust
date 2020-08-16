use crate::prelude::*;
use futures::{
  future::Future,
  future::FutureExt,
  task::{LocalSpawn, LocalSpawnExt, Spawn, SpawnExt},
};
use observable::of;
use std::marker::PhantomData;

/// Converts a `Future` to an observable sequence. Even though if the future
/// poll value has `Result::Err` type, also emit as a normal value, not trigger
/// to error handle.
///
/// ```rust
/// # use rxrust::prelude::*;
/// # use std::sync::{Arc, Mutex};
/// let res = Arc::new(Mutex::new(0));
/// let c_res = res.clone();
/// use futures::future;
/// observable::from_future(future::ready(1))
///   .to_shared()
///   .subscribe(move |v| {
///     *res.lock().unwrap() = v;
///   });
/// std::thread::sleep(std::time::Duration::new(1, 0));
/// assert_eq!(*c_res.lock().unwrap(), 1);
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
  S: Spawn,
{
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let f = self
      .future
      .map(move |v| SharedEmitter::emit(of::OfEmitter(v), subscriber));
    self.scheduler.spawn(f).unwrap();
  }
}

impl<Item, F, S> LocalEmitter<'static> for FutureEmitter<F, S>
where
  F: Future<Output = Item> + 'static,
  S: LocalSpawn,
{
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + 'static,
  {
    let f = self
      .future
      .map(move |v| LocalEmitter::emit(of::OfEmitter(v), subscriber));
    self.scheduler.spawn_local(f).unwrap();
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
    marker: PhantomData,
  })
}

#[derive(Clone)]
pub struct FutureResultEmitter<F, S, Item, Err> {
  future: F,
  scheduler: S,
  marker: PhantomData<(Item, Err)>,
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
  S: Spawn,
{
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let f = self.future.map(move |v| {
      SharedEmitter::emit(of::ResultEmitter(v.into()), subscriber)
    });
    self.scheduler.spawn(f).unwrap();
  }
}

impl<Item, Err, S, F> LocalEmitter<'static>
  for FutureResultEmitter<F, S, Item, Err>
where
  F: Future + 'static,
  <F as Future>::Output: Into<Result<Item, Err>>,
  S: LocalSpawn,
{
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + 'static,
  {
    let f = self.future.map(move |v| {
      LocalEmitter::emit(of::ResultEmitter(v.into()), subscriber)
    });
    self.scheduler.spawn_local(f).unwrap();
  }
}

#[test]
fn smoke() {
  use futures::future;
  use std::sync::Arc;
  let res = Arc::new(Mutex::new(0));
  let c_res = res.clone();
  {
    from_future_result(future::ok(1))
      .to_shared()
      .subscribe(move |v| {
        *res.lock().unwrap() = v;
      });
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(*c_res.lock().unwrap(), 1);
  }
  // from_future
  let res = c_res.clone();
  from_future(future::ready(2))
    .to_shared()
    .subscribe(move |v| {
      *res.lock().unwrap() = v;
    });
  std::thread::sleep(std::time::Duration::from_millis(10));
  assert_eq!(*c_res.lock().unwrap(), 2);
}
