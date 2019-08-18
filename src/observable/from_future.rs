use crate::observable::ObservableOnce;
use crate::prelude::*;
use futures::{executor::ThreadPool, future::FutureExt, task::SpawnExt};
use std::sync::Mutex;

lazy_static! {
  static ref THREAD_POOL: Mutex<ThreadPool> =
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
///     *res.lock().unwrap() = *v;
///   });
/// std::thread::sleep(std::time::Duration::new(1, 0));
/// assert_eq!(*c_res.lock().unwrap(), 1);
/// ```
/// If your `Future` poll an `Result` type value, and you want dispatch the
/// error by rxrust, you can use [`from_future_with_err`]
///
pub fn from_future<F, Item>(
  f: F,
) -> ObservableOnce<impl FnOnce(Box<dyn Observer<Item, ()> + Send>), Item, ()>
where
  F: FutureExt<Output = Item> + Send + 'static,
  Item: 'static,
{
  observable::once::<_, Item, ()>(move |mut subscriber| {
    let f = f.map(move |v| {
      if !subscriber.is_stopped() {
        subscriber.next(&v);
        subscriber.complete();
      }
    });
    THREAD_POOL.lock().unwrap().spawn(f).unwrap();
  })
}

/// Converts a `Future` to an observable sequence like [`from_future`].
/// But only work for which `Future::Output` is `Result` type, and `Result::Ok`
/// emit to next handle, and `Result::Err` as an error to handle.
pub fn from_future_with_err<F, Item, Err>(
  f: F,
) -> ObservableOnce<impl FnOnce(Box<dyn Observer<Item, Err> + Send>), Item, Err>
where
  F: FutureExt<Output = Result<Item, Err>> + Send + 'static,
  Item: 'static,
  Err: 'static,
{
  observable::once::<_, Item, Err>(move |mut subscriber| {
    let f = f.map(move |v| {
      if !subscriber.is_stopped() {
        match v {
          Ok(ref item) => {
            subscriber.next(&item);
            subscriber.complete();
          }
          Err(ref err) => subscriber.error(&err),
        };
      }
    });
    THREAD_POOL.lock().unwrap().spawn(f).unwrap();
  })
}

#[test]
fn smoke() {
  use std::sync::Arc;
  let res = Arc::new(Mutex::new(0));
  let c_res = res.clone();
  use futures::future;
  from_future_with_err::<_, _, ()>(future::ok(1)).subscribe(move |v| {
    *res.lock().unwrap() = *v;
  });
  std::thread::sleep(std::time::Duration::new(1, 0));
  assert_eq!(*c_res.lock().unwrap(), 1);
}
