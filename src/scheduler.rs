use crate::{
  prelude::*,
  rc::{MutArc, RcDeref, RcDerefMut},
};
use futures::{future::CatchUnwind, ready, FutureExt};
use pin_project_lite::pin_project;
use std::{
  any::Any,
  fmt,
  future::Future,
  mem::swap,
  panic::{self, AssertUnwindSafe},
  pin::Pin,
  task::{Context, Poll},
};

#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<'a, T> = futures::future::BoxFuture<'a, T>;
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<'a, T> = futures::future::LocalBoxFuture<'a, T>;

#[cfg(feature = "timer")]
fn new_timer(dur: Duration) -> BoxFuture<'static, ()> {
  #[cfg(not(target_arch = "wasm32"))]
  use futures_time::task::sleep;
  #[cfg(target_arch = "wasm32")]
  use gloo_timers::future::sleep;

  Box::pin(sleep(dur.into()).map(|_| ()))
}

#[cfg(not(feature = "timer"))]
pub static NEW_TIMER_FN: once_cell::sync::OnceCell<
  fn(Duration) -> BoxFuture<'static, ()>,
> = once_cell::sync::OnceCell::new();

#[cfg(not(feature = "timer"))]
fn new_timer(dur: Duration) -> BoxFuture<'static, ()> {
  NEW_TIMER_FN
    .get()
    .expect("you can enable the default timer by `timer` feature, or set yourself timer across function `new_timer_fn`")(dur)
}

pub struct TaskHandle<T>(MutArc<HandleInfo<T>>);
struct HandleInfo<T> {
  keep_running: bool,
  value: Option<Result<T, Box<dyn Any + Send>>>,
}

pub trait Scheduler<T>: Clone
where
  T: Future,
{
  fn schedule(&self, task: T, delay: Option<Duration>)
    -> TaskHandle<T::Output>;
}

pin_project! {
  pub struct OnceTask<Args, R> {
    func: fn(Args) -> R,
    args: Option<Args>,
  }
}

pin_project! {
  pub struct FutureTask<F: Future, Args, R> {
    #[pin]
    future: F,
    task: fn(F::Output, Args)->R,
    args: Option<Args>,
  }
}

pin_project! {
  pub struct RepeatTask<Args> {
    fur: BoxFuture<'static, ()>,
    interval: Duration,
    // the task to do and return if you want the task continue repeat.
    task: fn(&mut Args, usize)-> bool,
    args: Args,
    seq: usize,
  }
}

impl<Args, R> OnceTask<Args, R> {
  #[inline]
  pub fn new(func: fn(Args) -> R, args: Args) -> Self {
    OnceTask { func, args: Some(args) }
  }
}

impl<F: Future, Args, R> FutureTask<F, Args, R> {
  pub fn new(future: F, task: fn(F::Output, Args) -> R, args: Args) -> Self {
    Self { future, task, args: Some(args) }
  }
}

impl<Args, R: TaskReturn> Future for OnceTask<Args, R> {
  type Output = R;

  fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    let args = this.args.take().unwrap();
    Poll::Ready((*this.func)(args))
  }
}

impl<Args> Future for RepeatTask<Args> {
  type Output = NormalReturn<()>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    loop {
      ready!(self.fur.poll_unpin(cx));
      {
        let this = self.as_mut().project();
        if (*this.task)(this.args, *this.seq) {
          *this.seq += 1;
        } else {
          return Poll::Ready(NormalReturn::new(()));
        }
      }
      let mut fur = new_timer(self.interval);
      swap(&mut self.fur, &mut fur);
    }
  }
}

impl<F, Args, R: TaskReturn> Future for FutureTask<F, Args, R>
where
  F: Future,
{
  type Output = R;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match this.future.poll(cx) {
      Poll::Ready(v) => {
        let args = this.args.take().unwrap();
        Poll::Ready((*this.task)(v, args))
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

impl<Args> RepeatTask<Args> {
  pub fn new(
    dur: Duration,
    task: fn(&mut Args, usize) -> bool,
    args: Args,
  ) -> Self {
    Self {
      fur: new_timer(dur),
      interval: dur,
      task,
      args,
      seq: 0,
    }
  }
}

pub struct SubscribeReturn<T: Subscription>(T);
pub struct NormalReturn<T>(T);

impl<T> NormalReturn<T> {
  #[inline]
  pub fn new(v: T) -> Self {
    Self(v)
  }
}

impl<T: Subscription> SubscribeReturn<T> {
  #[inline]
  pub fn new(v: T) -> Self {
    Self(v)
  }
}

impl<T> TaskHandle<T> {
  pub fn value_handle(v: T) -> Self {
    Self(MutArc::own(HandleInfo {
      keep_running: true,
      value: Some(Ok(v)),
    }))
  }
}
trait TaskReturn {}

impl<T: Subscription> TaskReturn for SubscribeReturn<T> {}
impl<T> TaskReturn for NormalReturn<T> {}

impl<T: 'static> Subscription for TaskHandle<NormalReturn<T>> {
  #[inline]
  fn unsubscribe(self) {
    let mut inner = self.0.rc_deref_mut();
    inner.keep_running = false;
    inner.value.take();
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.0.rc_deref().value.is_some()
  }
}

impl<T: Subscription + 'static> Subscription
  for TaskHandle<SubscribeReturn<T>>
{
  fn unsubscribe(self) {
    let mut info = self.0.rc_deref_mut();
    info.keep_running = false;
    match info.value.take() {
      Some(Ok(v)) => v.0.unsubscribe(),
      Some(Err(e)) => panic::resume_unwind(e),
      None => {}
    }
  }

  #[inline]
  fn is_closed(&self) -> bool {
    let info = self.0.rc_deref();
    match info.value.as_ref() {
      Some(Ok(u)) => u.0.is_closed(),
      _ => false,
    }
  }
}

pin_project! {
  /// A future which sends its output to the corresponding `RemoteHandle`.
  /// Created by [`remote_handle`](crate::future::FutureExt::remote_handle).
  #[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
  struct Remote<Fut: Future> {
      handle_info: MutArc<HandleInfo<Fut::Output>>,
      #[pin]
      future: CatchUnwind<AssertUnwindSafe<Fut>>,
  }
}

impl<Fut: Future + fmt::Debug> fmt::Debug for Remote<Fut> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("Remote").field(&self.future).finish()
  }
}

impl<Fut: Future> Future for Remote<Fut> {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    let this = self.project();

    let mut info = this.handle_info.rc_deref_mut();
    if !info.keep_running {
      // Cancelled, bail out
      return Poll::Ready(());
    }
    info.value = Some(ready!(this.future.poll(cx)));

    Poll::Ready(())
  }
}

fn remote_handle<Fut: Future>(
  future: Fut,
) -> (Remote<Fut>, TaskHandle<Fut::Output>) {
  let handle =
    TaskHandle(MutArc::own(HandleInfo { keep_running: true, value: None }));

  // Unwind Safety: See the docs for RemoteHandle.
  let wrapped = Remote {
    future: AssertUnwindSafe(future).catch_unwind(),
    handle_info: handle.0.clone(),
  };

  (wrapped, handle)
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct WasmLocalScheduler;
#[cfg(feature = "futures-scheduler")]
pub use futures::executor::LocalPool as FuturesLocalSchedulerPool;
#[cfg(feature = "futures-scheduler")]
pub use futures::executor::LocalSpawner as FuturesLocalScheduler;
#[cfg(all(feature = "futures-scheduler", not(target_arch = "wasm32")))]
pub use futures::executor::ThreadPool as FuturesThreadPoolScheduler;
#[cfg(all(feature = "tokio-scheduler", not(target_arch = "wasm32")))]
pub use tokio::runtime::Handle as TokioScheduler;

macro_rules! impl_scheduler_method {
  ($spawn_macro: ident) => {
    fn schedule(
      &self,
      task: T,
      delay: Option<Duration>,
    ) -> TaskHandle<T::Output> {
      let fut = async move {
        if let Some(dur) = delay {
          new_timer(dur).await;
        }
        task.await
      };
      let (fut, handle) = remote_handle(fut);
      $spawn_macro!(self, fut);
      handle
    }
  };
}

#[cfg(feature = "futures-scheduler")]
macro_rules! futures_local_spawn {
  ($pool: ident, $future: ident) => {
    $pool.spawn_local($future).unwrap()
  };
}

#[cfg(target_arch = "wasm32")]
mod wasm_scheduler {
  use super::*;
  use futures::task::LocalSpawnExt;

  macro_rules! wasm_bindgen_spawn {
    ($pool: ident, $future: ident) => {
      wasm_bindgen_futures::spawn_local($future);
    };
  }

  impl<T> Scheduler<T> for WasmLocalScheduler
  where
    T: Future + 'static,
    T::Output: TaskReturn,
  {
    impl_scheduler_method!(wasm_bindgen_spawn);
  }

  #[cfg(feature = "futures-scheduler")]
  impl<T> Scheduler<T> for FuturesLocalScheduler
  where
    T: Future + 'static,
    T::Output: TaskReturn,
  {
    impl_scheduler_method!(futures_local_spawn);
  }
}

#[cfg(not(target_arch = "wasm32"))]
mod not_wasm_scheduler {
  use super::*;

  #[cfg(feature = "futures-scheduler")]
  mod futures_scheduler {
    use super::*;
    use futures::task::{LocalSpawnExt, SpawnExt};

    macro_rules! futures_pool_spawn {
      ($pool: ident, $future: ident) => {
        $pool.spawn($future).unwrap()
      };
    }

    impl<T> Scheduler<T> for FuturesThreadPoolScheduler
    where
      T: Future + Send + 'static,
      T::Output: TaskReturn + Send + 'static,
    {
      impl_scheduler_method!(futures_pool_spawn);
    }

    impl<T> Scheduler<T> for FuturesLocalScheduler
    where
      T: Future + 'static,
      T::Output: TaskReturn,
    {
      impl_scheduler_method!(futures_local_spawn);
    }
  }

  #[cfg(feature = "tokio-scheduler")]
  mod tokio_scheduler {
    use super::*;

    macro_rules! tokio_runtime_spawn {
      ($pool: ident, $future: ident) => {
        $pool.spawn($future)
      };
    }

    impl<T> Scheduler<T> for TokioScheduler
    where
      T: Future + Send + 'static,
      T::Output: TaskReturn + Send + 'static,
    {
      impl_scheduler_method!(tokio_runtime_spawn);
    }
  }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "tokio-scheduler"))]
mod test {
  use crate::{ops::complete_status::CompleteStatus, prelude::*};
  use bencher::Bencher;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};

  #[test]
  fn bench_pool() {
    do_bench_pool();
  }

  benchmark_group!(do_bench_pool, pool);

  fn pool(b: &mut Bencher) {
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let pool = ThreadPool::new().unwrap();
      let (o, status) = observable::from_iter(0..1000)
        .observe_on_threads(pool)
        .complete_status();
      o.subscribe(move |v| *c_last.lock().unwrap() = v);
      CompleteStatus::wait_for_end(status);

      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_local_thread() {
    do_bench_local_thread();
  }

  benchmark_group!(do_bench_local_thread, local_thread);

  fn local_thread(b: &mut Bencher) {
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let mut local = LocalPool::new();
      observable::from_iter(0..1000)
        .observe_on(local.spawner())
        .subscribe(move |v| *c_last.lock().unwrap() = v);
      local.run();
      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_tokio_basic() {
    do_bench_tokio_basic();
  }

  benchmark_group!(do_bench_tokio_basic, tokio_basic);

  fn tokio_basic(b: &mut Bencher) {
    use tokio::runtime;
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let local = runtime::Builder::new_current_thread().build().unwrap();
      let scheduler = local.handle().clone();

      let (o, status) = observable::from_iter(0..1000)
        .observe_on_threads(scheduler)
        .complete_status();
      o.subscribe(move |v| *c_last.lock().unwrap() = v);
      CompleteStatus::wait_for_end(status);

      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_tokio_thread() {
    do_bench_tokio_thread();
  }

  benchmark_group!(do_bench_tokio_thread, tokio_thread);

  fn tokio_thread(b: &mut Bencher) {
    use tokio::runtime;
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let pool = runtime::Runtime::new().unwrap().handle().clone();
      let (o, status) = observable::from_iter(0..1000)
        .observe_on_threads(pool)
        .complete_status();
      o.subscribe(move |v| *c_last.lock().unwrap() = v);
      CompleteStatus::wait_for_end(status);
      *last.lock().unwrap()
    })
  }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod test {
  use crate::prelude::*;
  use wasm_bindgen_test::*;

  #[wasm_bindgen_test]
  fn test_local() {
    let mut container = Vec::new();
    observable::from_iter(1..=5).subscribe(|val| container.push(val));
    assert_eq!(container, vec![1, 2, 3, 4, 5]);
  }
}
