use crate::prelude::*;
use futures::{
  executor::ThreadPool, future::Future, future::FutureExt, task::SpawnExt,
};
use observable::of;
use std::marker::PhantomData;
use std::sync::Mutex;

lazy_static! {
  pub static ref DEFAULT_RUNTIME: Mutex<ThreadPool> =
    Mutex::new(ThreadPool::new().unwrap());
}

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
/// let _guard = observable::from_future(future::ready(1))
///   .subscribe(move |v| {
///     *res.lock().unwrap() = v;
///   });
/// std::thread::sleep(std::time::Duration::new(1, 0));
/// assert_eq!(*c_res.lock().unwrap(), 1);
/// ```
/// If your `Future` poll an `Result` type value, and you want dispatch the
/// error by rxrust, you can use [`from_future_result`]
///
pub fn from_future<F, Item>(f: F) -> ObservableBase<FutureEmitter<F>>
where
  F: Future<Output = Item> + Send + Clone + Sync + 'static,
{
  ObservableBase::new(FutureEmitter(f))
}

#[derive(Clone)]
pub struct FutureEmitter<F>(F);

impl<Item, F> SharedEmitter for FutureEmitter<F>
where
  F: Future<Output = Item> + Send + Sync + 'static,
{
  type Item = Item;
  type Err = ();
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let fmapped = (self.0).map(move |v| of::OfEmitter(v).emit(subscriber));
    DEFAULT_RUNTIME.lock().unwrap().spawn(fmapped).unwrap();
  }
}

auto_impl_shared_emitter!(FutureEmitter<F>, <F>);

/// Converts a `Future` to an observable sequence like [`from_future`].
/// But only work for which `Future::Output` is `Result` type, and `Result::Ok`
/// emit to next handle, and `Result::Err` as an error to handle.
pub fn from_future_result<F, Item, Err>(
  f: F,
) -> ObservableBase<FutureResultEmitter<F, Item, Err>>
where
  Err: Send + Sync + 'static,
  Item: Send + Sync + 'static,
  F: Future + Send + Clone + Sync + 'static,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  ObservableBase::new(FutureResultEmitter(f, PhantomData))
}

#[derive(Clone)]
pub struct FutureResultEmitter<F, Item, Err>(F, PhantomData<(Item, Err)>);

impl<Item, Err, F> SharedEmitter for FutureResultEmitter<F, Item, Err>
where
  Item: Send + Sync + 'static,
  Err: Send + Sync + 'static,
  F: Future + Send + Clone + Sync + 'static,
  <F as Future>::Output: Into<Result<Item, Err>>,
{
  type Item = Item;
  type Err = Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let fmapped =
      (self.0).map(move |v| of::ResultEmitter(v.into()).emit(subscriber));
    DEFAULT_RUNTIME.lock().unwrap().spawn(fmapped).unwrap();
  }
}

auto_impl_shared_emitter!(FutureResultEmitter<F, Item, Err,>, <F, Item, Err>);

#[test]
fn smoke() {
  use futures::future;
  use std::sync::Arc;
  let res = Arc::new(Mutex::new(0));
  let c_res = res.clone();
  {
    let _guard =
      from_future_result(future::ok(1))
        .shared()
        .subscribe(move |v| {
          *res.lock().unwrap() = v;
        });
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(*c_res.lock().unwrap(), 1);
  }
  // from_future
  let res = c_res.clone();
  let _guard = from_future(future::ready(2)).shared().subscribe(move |v| {
    *res.lock().unwrap() = v;
  });
  std::thread::sleep(std::time::Duration::from_millis(10));
  assert_eq!(*c_res.lock().unwrap(), 2);
}
