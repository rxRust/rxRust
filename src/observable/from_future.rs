use crate::ops::SharedOp;
use crate::prelude::*;
use futures::{
  executor::ThreadPool, future::Future, future::FutureExt, task::SpawnExt,
};
use observable::ObservableFromFn;
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
/// observable::from_future(future::ready(1))
///   .subscribe(move |v| {
///     *res.lock().unwrap() = v;
///   });
/// std::thread::sleep(std::time::Duration::new(1, 0));
/// assert_eq!(*c_res.lock().unwrap(), 1);
/// ```
/// If your `Future` poll an `Result` type value, and you want dispatch the
/// error by rxrust, you can use [`from_future_with_err`]
///
pub fn from_future<O, U, F, Item>(
  f: F,
) -> ops::SharedOp<
  ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>,
>
where
  Item: 'static,
  O: Observer<Item, ()> + Send + 'static,
  U: SubscriptionLike + Send + 'static,
  F: Future<Output = Item> + Send + Clone + Sync + 'static,
{
  observable::create(move |mut subscriber| {
    let fmapped = f.map(move |v| {
      if !subscriber.is_closed() {
        subscriber.next(v);
        subscriber.complete();
      }
    });
    DEFAULT_RUNTIME.lock().unwrap().spawn(fmapped).unwrap();
  })
  .to_shared()
}

/// Converts a `Future` to an observable sequence like [`from_future`].
/// But only work for which `Future::Output` is `Result` type, and `Result::Ok`
/// emit to next handle, and `Result::Err` as an error to handle.
pub fn from_future_with_err<O, U, F, Item, Error>(
  f: F,
) -> SharedOp<
  ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, Error>,
>
where
  Error: 'static,
  Item: 'static,
  O: Observer<Item, Error> + Send + 'static,
  U: SubscriptionLike + Send + 'static,
  F: Future + Send + Clone + Sync + 'static,
  <F as Future>::Output: Into<Result<Item, Error>>,
{
  observable::create(move |mut subscriber| {
    let fmapped = f.map(move |v| {
      if !subscriber.is_closed() {
        match v.into() {
          Ok(item) => {
            subscriber.next(item);
            subscriber.complete();
          }
          Err(err) => subscriber.error(err),
        };
      }
    });
    DEFAULT_RUNTIME.lock().unwrap().spawn(fmapped).unwrap();
  })
  .to_shared()
}

#[test]
fn smoke() {
  use futures::future;
  use std::sync::Arc;
  let res = Arc::new(Mutex::new(0));
  let c_res = res.clone();
  {
    from_future_with_err(future::ok(1)).subscribe(move |v| {
      *res.lock().unwrap() = v;
    });
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(*c_res.lock().unwrap(), 1);
  }
  // from_future
  let res = c_res.clone();
  from_future(future::ready(2)).subscribe(move |v| {
    *res.lock().unwrap() = v;
  });
  std::thread::sleep(std::time::Duration::from_millis(10));
  assert_eq!(*c_res.lock().unwrap(), 2);
}
