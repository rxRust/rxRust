use crate::prelude::*;
use futures::{future::RemoteHandle, ready, FutureExt, Stream};
use pin_project_lite::pin_project;
use std::{
  cell::RefCell,
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

pub struct TaskHandle<T>(RefCell<Option<InnerHandle<T>>>);

enum InnerHandle<T> {
  Handle(RemoteHandle<T>),
  Value(T),
}

pub trait Scheduler<T>: Clone
where
  T: Future,
{
  fn schedule(
    &self,
    task: T,
    delay: Option<std::time::Duration>,
  ) -> TaskHandle<T::Output>;
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

#[cfg(target_arch = "wasm32")]
type Interval = gloo_timers::future::IntervalStream;
#[cfg(not(target_arch = "wasm32"))]
type Interval = futures_time::stream::Interval;

pin_project! {
  pub struct RepeatTask<Args> {
    #[pin]
    interval: Interval,
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
    let mut this = self.as_mut().project();

    loop {
      let instant = ready!(this.interval.as_mut().poll_next(cx));
      match instant {
        Some(_) if (*this.task)(this.args, *this.seq) => {
          *this.seq += 1;
        }
        _ => return Poll::Ready(NormalReturn::new(())),
      }
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
    dur: std::time::Duration,
    task: fn(&mut Args, usize) -> bool,
    args: Args,
  ) -> Self {
    #[cfg(target_arch = "wasm32")]
    let interval = Interval::new(dur.as_millis() as u32);
    #[cfg(not(target_arch = "wasm32"))]
    let interval = futures_time::stream::interval(dur.into());

    Self { interval, task, args, seq: 0 }
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
  pub fn value(v: T) -> Self {
    Self(RefCell::new(Some(InnerHandle::Value(v))))
  }

  fn remote_handle(handle: RemoteHandle<T>) -> Self {
    Self(RefCell::new(Some(InnerHandle::Handle(handle))))
  }
}
trait TaskReturn {}

impl<T: Subscription> TaskReturn for SubscribeReturn<T> {}
impl<T> TaskReturn for NormalReturn<T> {}

impl<T: 'static> Subscription for TaskHandle<NormalReturn<T>> {
  #[inline]
  fn unsubscribe(self) {
    self.0.borrow_mut().take();
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.sync_remote();
    self
      .0
      .borrow()
      .as_ref()
      .map_or(true, |h| matches!(h, InnerHandle::Value(_)))
  }
}

impl<T: Subscription + 'static> Subscription
  for TaskHandle<SubscribeReturn<T>>
{
  fn unsubscribe(self) {
    self.sync_remote();
    if let Some(InnerHandle::Value(u)) = self.0.borrow_mut().take() {
      u.0.unsubscribe()
    }
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.sync_remote();

    self.0.borrow().as_ref().map_or(true, |inner| match inner {
      InnerHandle::Handle(_) => false,
      InnerHandle::Value(u) => u.0.is_closed(),
    })
  }
}

impl<T: 'static> TaskHandle<T> {
  fn sync_remote(&self) {
    let inner = &mut *self.0.borrow_mut();

    if let Some(InnerHandle::Handle(handler)) = inner {
      let waker = unsafe { Waker::from_raw(MOCK_RAW_WAKER) };
      let mut cx = Context::from_waker(&waker);
      let future = Pin::new(handler);
      if let Poll::Ready(value) = future.poll(&mut cx) {
        *inner = Some(InnerHandle::Value(value));
      }
    }
  }
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(
  |data: *const ()| RawWaker::new(data, &VTABLE),
  |_data: *const ()| (),
  |_data: *const ()| (),
  |_data: *const ()| (),
);

const MOCK_RAW_WAKER: RawWaker =
  RawWaker::new((&VTABLE as *const RawWakerVTable).cast(), &VTABLE);

impl<T> Drop for TaskHandle<T> {
  #[inline]
  fn drop(&mut self) {
    if let Some(InnerHandle::Handle(handle)) = self.0.borrow_mut().take() {
      handle.forget()
    }
  }
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
      delay: Option<std::time::Duration>,
    ) -> TaskHandle<T::Output> {
      let fut = async move {
        if let Some(dur) = delay {
          let dur: Duration = dur.into();
          sleep(dur.into()).await;
        }
        task.await
      };
      let (fut, handle) = fut.remote_handle();
      $spawn_macro!(self, fut);
      TaskHandle::remote_handle(handle)
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
  use gloo_timers::future::sleep;
  use std::time::Duration;

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
  use futures_time::{task::sleep, time::Duration};

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
        .observe_on(pool)
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
        .observe_on(scheduler)
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
        .observe_on(pool)
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
