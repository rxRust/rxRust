use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct BufferWithCountOp<S> {
  pub(crate) source: S,
  pub(crate) count: usize,
}

#[doc(hidden)]
macro_rules! buffer_op_observable_impl {
  ($ty: ident, $host: ident$(, $lf: lifetime)?$(, $generics: ident) *) => {
    impl<$($lf, )? $host, $($generics ,)*> Observable
    for $ty<$($lf, )? $host, $($generics ,)*>
    where
      $host: Observable
    {
      type Item = Vec<$host::Item>;
      type Err = $host::Err;
    }
  }
}

buffer_op_observable_impl!(BufferWithCountOp, S);

impl<'a, S> LocalObservable<'a> for BufferWithCountOp<S>
where
  S: LocalObservable<'a>,
  S::Item: 'a,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self
      .source
      .actual_subscribe(BufferWithCountObserver::new(observer, self.count))
  }
}

impl<S> SharedObservable for BufferWithCountOp<S>
where
  S: SharedObservable,
  S::Item: Send + Sync + 'static,
{
  type Unsub = S::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self
      .source
      .actual_subscribe(BufferWithCountObserver::new(observer, self.count))
  }
}

#[derive(Clone)]
pub struct BufferWithCountObserver<O, Item> {
  observer: O,
  buffer: Vec<Item>,
  count: usize,
}

impl<O, Item> BufferWithCountObserver<O, Item> {
  fn new(observer: O, count: usize) -> BufferWithCountObserver<O, Item> {
    BufferWithCountObserver {
      observer,
      buffer: vec![],
      count,
    }
  }
}

impl<O, Item, Err> Observer for BufferWithCountObserver<O, Item>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    self.buffer.push(value);

    if self.buffer.len() >= self.count {
      let buffer = std::mem::take(&mut self.buffer);
      self.observer.next(buffer);
    }
  }

  fn complete(&mut self) {
    if !self.buffer.is_empty() {
      let buffer = std::mem::take(&mut self.buffer);
      self.observer.next(buffer);
    }

    self.observer.complete();
  }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }
}

#[derive(Clone)]
pub struct BufferWithTimeOp<Source, Scheduler> {
  pub(crate) source: Source,
  pub(crate) time: Duration,
  pub(crate) scheduler: Scheduler,
}

buffer_op_observable_impl!(BufferWithTimeOp, S, Scheduler);

impl<Source, Scheduler> LocalObservable<'static>
  for BufferWithTimeOp<Source, Scheduler>
where
  Source: LocalObservable<'static>,
  Source::Item: 'static,
  Scheduler: LocalScheduler + 'static,
{
  type Unsub = Source::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  {
    self.source.actual_subscribe(BufferWithTimeObserver::new(
      observer,
      self.time,
      self.scheduler,
    ))
  }
}

#[derive(Clone)]
pub struct BufferWithTimeObserver<O, Item> {
  observer: Rc<RefCell<O>>,
  buffer: Rc<RefCell<Vec<Item>>>,
  handle: SpawnHandle,
}

impl<O, Item> BufferWithTimeObserver<O, Item>
where
  O: Observer<Item = Vec<Item>> + 'static,
  Item: 'static,
{
  fn new<S>(
    observer: O,
    time: Duration,
    scheduler: S,
  ) -> BufferWithTimeObserver<O, Item>
  where
    S: LocalScheduler + 'static,
  {
    let observer = Rc::new(RefCell::new(observer));
    let mut observer_c = observer.clone();

    let buffer = Rc::new(RefCell::new(vec![]));
    let buffer_c = buffer.clone();

    let handle = scheduler.schedule_repeating(
      move |_| {
        if !buffer_c.borrow().is_empty() {
          observer_c.next(buffer_c.take());
        }
      },
      time,
      None,
    );

    BufferWithTimeObserver {
      observer,
      buffer,
      handle,
    }
  }
}

#[doc(hidden)]
macro_rules! complete_time_impl_local {
  ($buffer:tt, $observer:tt, $handle:tt) => {
    fn complete(&mut self) {
      let buffer = self.$buffer.take();
      if !buffer.is_empty() {
        self.$observer.next(buffer);
      }

      self.$handle.unsubscribe();
      self.$observer.complete();
    }
  };
}

impl<O, Item, Err> Observer for BufferWithTimeObserver<O, Item>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    self.buffer.borrow_mut().push(value);
  }

  fn error(&mut self, err: Self::Err) {
    self.handle.unsubscribe();
    self.observer.error(err);
  }

  complete_time_impl_local!(buffer, observer, handle);
}

impl<Source, Scheduler> SharedObservable for BufferWithTimeOp<Source, Scheduler>
where
  Source: SharedObservable,
  <Source as Observable>::Item: Send + Sync + 'static,
  Scheduler: SharedScheduler,
{
  type Unsub = Source::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self
      .source
      .actual_subscribe(BufferWithTimeObserverShared::new(
        observer,
        self.time,
        self.scheduler,
      ))
  }
}

#[derive(Clone)]
pub struct BufferWithTimeObserverShared<O, Item> {
  observer: Arc<Mutex<O>>,
  buffer: Arc<Mutex<Vec<Item>>>,
  handle: SpawnHandle,
}

impl<O, Item> BufferWithTimeObserverShared<O, Item>
where
  O: Observer<Item = Vec<Item>> + Send + Sync + 'static,
  Item: Send + Sync + 'static,
{
  fn new<S>(
    observer: O,
    time: Duration,
    scheduler: S,
  ) -> BufferWithTimeObserverShared<O, Item>
  where
    S: SharedScheduler,
  {
    let observer = Arc::new(Mutex::new(observer));
    let mut observer_c = observer.clone();

    let buffer = Arc::new(Mutex::new(vec![]));
    let buffer_c = buffer.clone();

    let handle = scheduler.schedule_repeating(
      move |_| {
        let mut buffer = buffer_c.lock().unwrap();
        let buffer = std::mem::take(&mut *buffer);
        if !buffer.is_empty() {
          observer_c.next(buffer);
        }
      },
      time,
      None,
    );

    BufferWithTimeObserverShared {
      observer,
      buffer,
      handle,
    }
  }
}

#[doc(hidden)]
macro_rules! complete_time_impl_shared {
  ($buffer:tt, $observer:tt, $handle:tt) => {
    fn complete(&mut self) {
      let mut buffer = self.$buffer.lock().unwrap();
      let buffer = std::mem::take(&mut *buffer);

      if !buffer.is_empty() {
        self.$observer.next(buffer);
      }

      self.$handle.unsubscribe();
      self.$observer.complete();
    }
  };
}

impl<O, Item, Err> Observer for BufferWithTimeObserverShared<O, Item>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    let mut buffer = self.buffer.lock().unwrap();
    (*buffer).push(value);
  }

  fn error(&mut self, err: Self::Err) {
    self.handle.unsubscribe();
    self.observer.error(err);
  }

  complete_time_impl_shared!(buffer, observer, handle);
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerOp<Source, Scheduler> {
  pub(crate) source: Source,
  pub(crate) count: usize,
  pub(crate) time: Duration,
  pub(crate) scheduler: Scheduler,
}

buffer_op_observable_impl!(BufferWithCountOrTimerOp, S, Scheduler);

impl<Source, Scheduler> LocalObservable<'static>
  for BufferWithCountOrTimerOp<Source, Scheduler>
where
  Source: LocalObservable<'static>,
  Source::Item: 'static,
  Scheduler: LocalScheduler + 'static,
{
  type Unsub = Source::Unsub;

  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + 'static,
  >(
    self,
    observer: O,
  ) -> Self::Unsub {
    self
      .source
      .actual_subscribe(BufferWithCountOrTimerObserver::new(
        observer,
        self.count,
        self.time,
        self.scheduler,
      ))
  }
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerObserver<O, Item> {
  observer: Rc<RefCell<O>>,
  buffer: Rc<RefCell<Vec<Item>>>,
  count: usize,
  handle: SpawnHandle,
}

impl<O, Item> BufferWithCountOrTimerObserver<O, Item> {
  fn new<S>(observer: O, count: usize, time: Duration, scheduler: S) -> Self
  where
    O: Observer<Item = Vec<Item>> + 'static,
    Item: 'static,
    S: LocalScheduler + 'static,
  {
    let observer = Rc::new(RefCell::new(observer));
    let mut observer_c = observer.clone();

    let buffer = Rc::new(RefCell::new(vec![]));
    let buffer_c = buffer.clone();

    let handle = scheduler.schedule_repeating(
      move |_| {
        if buffer_c.borrow().is_empty() {
          observer_c.next(buffer_c.take());
        }
      },
      time,
      None,
    );

    BufferWithCountOrTimerObserver {
      observer,
      buffer,
      count,
      handle,
    }
  }
}

impl<O, Item, Err> Observer for BufferWithCountOrTimerObserver<O, Item>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    self.buffer.borrow_mut().push(value);

    if self.buffer.borrow().len() >= self.count {
      let buffer = self.buffer.take();
      self.observer.borrow_mut().next(buffer);
    }
  }

  fn error(&mut self, err: Self::Err) {
    self.handle.unsubscribe();
    self.observer.error(err);
  }

  complete_time_impl_local!(buffer, observer, handle);
}

impl<Source, Scheduler> SharedObservable
  for BufferWithCountOrTimerOp<Source, Scheduler>
where
  Source: SharedObservable,
  Source::Item: Send + Sync + 'static,
  Scheduler: SharedScheduler,
{
  type Unsub = Source::Unsub;

  fn actual_subscribe<
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  >(
    self,
    observer: O,
  ) -> Self::Unsub {
    self
      .source
      .actual_subscribe(BufferWithCountOrTimerObserverShared::new(
        observer,
        self.count,
        self.time,
        self.scheduler,
      ))
  }
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerObserverShared<O, Item> {
  observer: Arc<Mutex<O>>,
  buffer: Arc<Mutex<Vec<Item>>>,
  count: usize,
  handle: SpawnHandle,
}

impl<O, Item> BufferWithCountOrTimerObserverShared<O, Item> {
  fn new<S>(observer: O, count: usize, time: Duration, scheduler: S) -> Self
  where
    O: Observer<Item = Vec<Item>> + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    S: SharedScheduler,
  {
    let observer = Arc::new(Mutex::new(observer));
    let mut observer_c = observer.clone();

    let buffer = Arc::new(Mutex::new(vec![]));
    let buffer_c = buffer.clone();

    let handle = scheduler.schedule_repeating(
      move |_| {
        let mut buffer = buffer_c.lock().unwrap();
        if !buffer.is_empty() {
          let buffer = std::mem::take(&mut *buffer);
          observer_c.next(buffer);
        }
      },
      time,
      None,
    );

    BufferWithCountOrTimerObserverShared {
      observer,
      buffer,
      count,
      handle,
    }
  }
}

impl<O, Item, Err> Observer for BufferWithCountOrTimerObserverShared<O, Item>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    let mut buffer = self.buffer.lock().unwrap();
    (*buffer).push(value);

    if buffer.len() >= self.count {
      let buffer = std::mem::take(&mut *buffer);
      self.observer.next(buffer);
    }
  }

  fn error(&mut self, err: Self::Err) {
    self.handle.unsubscribe();
    self.observer.error(err);
  }

  complete_time_impl_shared!(buffer, observer, handle);
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  #[test]
  fn it_shall_buffer_with_count() {
    let expected =
      vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7], vec![8, 9]];
    let mut actual = vec![];
    observable::from_iter(0..10)
      .buffer_with_count(2)
      .subscribe(|vec| actual.push(vec));

    assert_eq!(expected, actual);
  }

  #[test]
  fn it_shall_buffer_with_count_shared() {
    let expected =
      vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7], vec![8, 9]];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();
    observable::from_iter(0..10)
      .buffer_with_count(2)
      .into_shared()
      .subscribe(move |vec| actual_c.lock().unwrap().push(vec));

    assert_eq!(expected, *actual.lock().unwrap());
  }

  #[test]
  fn it_shall_emit_buffer_on_completed() {
    let expected = vec![vec![0, 1], vec![2, 3], vec![4]];
    let mut actual = vec![];

    let is_completed = Rc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::create(|mut subscriber| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.complete();
    })
    .buffer_with_count(2)
    .subscribe_complete(
      |vec| actual.push(vec),
      move || is_completed_c.store(true, Ordering::Relaxed),
    );

    assert_eq!(expected, actual);
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_discard_buffer_on_error() {
    let expected = vec![vec![0, 1], vec![2, 3]];
    let mut actual = vec![];
    let mut err_called = false;

    observable::create(|mut subscriber| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.error(());
    })
    .buffer_with_count(2)
    .subscribe_err(|vec| actual.push(vec), |_| err_called = true);

    assert_eq!(expected, actual);
    assert!(err_called);
  }

  #[test]
  fn it_shall_buffer_with_time_local() {
    let mut local = LocalPool::new();

    let expected = vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();

    observable::from_iter(0..10)
      .buffer_with_time(Duration::from_millis(500), local.spawner())
      .subscribe(move |vec| actual_c.borrow_mut().push(vec));

    local.run();

    // this can't be really tested as local scheduler runs on a single thread
    assert_eq!(expected, *actual.borrow());
  }

  #[test]
  fn it_shall_not_block_with_error_on_time_local() {
    let mut local = LocalPool::new();

    observable::create(|mut subscriber| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.error(());
    })
    .buffer_with_time(Duration::from_millis(500), local.spawner())
    .subscribe(|_| {});

    // if this call blocks execution, the observer's handle has not been
    // unsubscribed
    local.run();
  }

  #[test]
  fn it_shall_buffer_with_time_shared() {
    let pool = ThreadPool::new().unwrap();

    let expected = vec![vec![0, 1, 2], vec![3, 4, 5, 6]];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::create(|mut subscriber| {
      let sleep = Duration::from_millis(100);
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      std::thread::sleep(sleep);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.next(5);
      subscriber.next(6);
      subscriber.complete();
    })
    .buffer_with_time(Duration::from_millis(50), pool)
    .into_shared()
    .subscribe_blocking_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      |()| {},
      move || is_completed_c.store(true, Ordering::Relaxed),
    );

    assert_eq!(expected, *actual.lock().unwrap());
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_not_emit_buffer_with_time_on_error() {
    let pool = ThreadPool::new().unwrap();

    let expected = vec![vec![0, 1, 2]];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();

    let error_called = Arc::new(AtomicBool::new(false));
    let error_called_c = error_called.clone();

    observable::create(|mut subscriber| {
      let sleep = Duration::from_millis(100);
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      std::thread::sleep(sleep);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.error(());
    })
    .buffer_with_time(Duration::from_millis(50), pool)
    .into_shared()
    .subscribe_blocking_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      move |_| error_called_c.store(true, Ordering::Relaxed),
      || {},
    );

    assert_eq!(expected, *actual.lock().unwrap());
    assert!(error_called.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_buffer_with_count_and_time() {
    let mut local = LocalPool::new();

    let expected =
      vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7], vec![8, 9]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();

    observable::from_iter(0..10)
      .buffer_with_count_and_time(
        2,
        Duration::from_millis(500),
        local.spawner(),
      )
      .subscribe(move |vec| actual_c.borrow_mut().push(vec));

    local.run();

    // this can't be really tested as local scheduler runs on a single thread
    assert_eq!(expected, *actual.borrow());
  }

  #[test]
  fn it_shall_buffer_with_count_and_time_on_error() {
    let mut local = LocalPool::new();

    let expected = vec![vec![0, 1]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();

    let error_called = Rc::new(AtomicBool::new(false));
    let error_called_c = error_called.clone();

    observable::create(|mut subscriber| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.error(());
      subscriber.next(3);
      subscriber.next(4);
    })
    .buffer_with_count_and_time(2, Duration::from_millis(500), local.spawner())
    .subscribe_err(
      move |vec| actual_c.borrow_mut().push(vec),
      move |_| error_called_c.store(true, Ordering::Relaxed),
    );

    local.run();

    assert_eq!(expected, *actual.borrow());
    assert!(error_called.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_buffer_with_count_or_time_shared() {
    let pool = ThreadPool::new().unwrap();

    let expected = vec![vec![0, 1], vec![2], vec![3, 4]];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::create(|mut subscriber| {
      let sleep = Duration::from_millis(100);
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      std::thread::sleep(sleep);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.complete();
    })
    .buffer_with_count_and_time(2, Duration::from_millis(50), pool)
    .into_shared()
    .subscribe_blocking_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      |()| {},
      move || is_completed_c.store(true, Ordering::Relaxed),
    );

    assert_eq!(expected, *actual.lock().unwrap());
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_buffer_with_count_or_time_shared_on_error() {
    let pool = ThreadPool::new().unwrap();

    let expected = vec![vec![0, 1], vec![2]];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();

    let error_called = Arc::new(AtomicBool::new(false));
    let error_called_c = error_called.clone();

    observable::create(|mut subscriber| {
      let sleep = Duration::from_millis(100);
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      std::thread::sleep(sleep);
      subscriber.next(3);
      subscriber.error(());
      subscriber.next(4);
    })
    .buffer_with_count_and_time(2, Duration::from_millis(50), pool)
    .into_shared()
    .subscribe_blocking_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      move |_| error_called_c.store(true, Ordering::Relaxed),
      || {},
    );

    assert_eq!(expected, *actual.lock().unwrap());
    assert!(error_called.load(Ordering::Relaxed));
  }
}
