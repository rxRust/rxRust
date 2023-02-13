use crate::{
  prelude::*,
  rc::{MutArc, RcDerefMut},
  scheduler::{NormalReturn, OnceTask, Scheduler},
};
#[derive(Clone)]
pub struct ObserveOnOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

impl<S, SD, Item, Err, O> Observable<Item, Err, O> for ObserveOnOp<S, SD>
where
  S: Observable<Item, Err, ObserveOnObserver<O, SD>>,
  O: Observer<Item, Err>,
  SD: Scheduler<OnceTask<(MutArc<Option<O>>, Item, RcHandler), NormalReturn<()>>>
    + Scheduler<OnceTask<(MutArc<Option<O>>, Err, RcHandler), NormalReturn<()>>>
    + Scheduler<OnceTask<(MutArc<Option<O>>, RcHandler), NormalReturn<()>>>,
  S::Unsub: Send + 'static,
{
  type Unsub = MultiSubscriptionThreads;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let mut unsub = MultiSubscriptionThreads::default();
    let observer = ObserveOnObserver {
      observer: MutArc::own(Some(observer)),
      scheduler: self.scheduler,
      subscription: unsub.clone(),
    };
    let u = self.source.actual_subscribe(observer);
    unsub.append(BoxSubscriptionThreads::new(u));
    unsub
  }
}

impl<S, SD, Item, Err> ObservableExt<Item, Err> for ObserveOnOp<S, SD> where
  S: ObservableExt<Item, Err>
{
}

pub struct ObserveOnObserver<O, SD> {
  observer: MutArc<Option<O>>,
  scheduler: SD,
  subscription: MultiSubscriptionThreads,
}

type RcHandler = MutArc<Option<TaskHandle<NormalReturn<()>>>>;
macro_rules! schedule_task {
  ($this: ident, $task_fn: ident, $($args: expr),*) => {{

    if !$this.subscription.is_closed() {
      let proxy_handler = MutArc::own(None);
      let handler = $this
        .scheduler
        .schedule(OnceTask::new($task_fn, ($($args,)* proxy_handler.clone())), None);
      *proxy_handler.rc_deref_mut() = Some(handler);
      $this.subscription.append(BoxSubscriptionThreads::new(proxy_handler));
    }

  }};
}

impl<Item, Err, O, SD> Observer<Item, Err> for ObserveOnObserver<O, SD>
where
  O: Observer<Item, Err>,
  SD: Scheduler<OnceTask<(MutArc<Option<O>>, Item, RcHandler), NormalReturn<()>>>
    + Scheduler<OnceTask<(MutArc<Option<O>>, Err, RcHandler), NormalReturn<()>>>
    + Scheduler<OnceTask<(MutArc<Option<O>>, RcHandler), NormalReturn<()>>>,
{
  fn next(&mut self, value: Item) {
    schedule_task!(self, next_task, self.observer.clone(), value);
  }

  fn error(mut self, err: Err) {
    schedule_task!(self, err_task, self.observer.clone(), err);
  }

  fn complete(mut self) {
    schedule_task!(self, complete_task, self.observer.clone())
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

fn next_task<Item, Err, O>(
  (mut observer, item, handler): (O, Item, RcHandler),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  handler.rc_deref_mut().take();
  observer.next(item);
  NormalReturn::new(())
}

fn err_task<Item, Err, O>(
  (observer, err, handler): (O, Err, RcHandler),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  handler.rc_deref_mut().take();
  observer.error(err);
  NormalReturn::new(())
}

fn complete_task<Item, Err, O>(
  (observer, handler): (O, RcHandler),
) -> NormalReturn<()>
where
  O: Observer<Item, Err>,
{
  handler.rc_deref_mut().take();
  observer.complete();
  NormalReturn::new(())
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::LocalPool;
  #[cfg(not(target_arch = "wasm32"))]
  use futures::executor::ThreadPool;
  #[cfg(not(target_arch = "wasm32"))]
  use std::collections::HashSet;
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::atomic::{AtomicBool, Ordering};
  #[cfg(not(target_arch = "wasm32"))]
  use std::sync::{Arc, Mutex};
  #[cfg(not(target_arch = "wasm32"))]
  use std::thread;
  #[cfg(not(target_arch = "wasm32"))]
  use std::time::Duration;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let v = Rc::new(RefCell::new(0));
    let v_c = v.clone();
    let mut local = LocalPool::new();
    observable::of(1)
      .observe_on(local.spawner())
      .subscribe(move |i| *v_c.borrow_mut() = i);
    local.run();

    assert_eq!(*v.borrow(), 1);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn switch_thread() {
    use crate::ops::complete_status::CompleteStatus;

    let id = thread::spawn(move || {}).thread().id();
    let changed_thread = Arc::new(AtomicBool::default());
    let c_changed_thread = changed_thread.clone();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(HashSet::new()));
    let thread_clone = observe_thread.clone();

    let pool = ThreadPool::builder().pool_size(100).create().unwrap();

    let (o, status) = observable::create(|mut p: SubscriberThreads<_>| {
      while !changed_thread.load(Ordering::Relaxed) {
        p.next(());
        *emit_thread.lock().unwrap() = thread::current().id();
      }
      p.complete();
    })
    .observe_on(pool)
    .complete_status();
    let _ = o.subscribe(move |_v| {
      let mut thread = observe_thread.lock().unwrap();
      thread.insert(thread::current().id());

      c_changed_thread.store(thread.len() > 1, Ordering::Relaxed);
    });

    CompleteStatus::wait_for_end(status);

    let current_id = thread::current().id();
    assert_eq!(*emit_thread.lock().unwrap(), current_id);
    let thread = thread_clone.lock().unwrap();
    assert!(thread.len() > 1);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn pool_unsubscribe() {
    let scheduler = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .delay_threads(Duration::from_millis(10), scheduler.clone())
      .observe_on(scheduler)
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_observe_on);

  fn bench_observe_on(b: &mut bencher::Bencher) {
    b.iter(smoke);
  }
}
