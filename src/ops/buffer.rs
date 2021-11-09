use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
use std::{ops::DerefMut, time::Duration};

#[derive(Clone)]
pub struct BufferWithCountOp<S> {
  pub(crate) source: S,
  pub(crate) count: usize,
}

impl<S: Observable> Observable for BufferWithCountOp<S> {
  type Item = Vec<S::Item>;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> BufferWithCountOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self
    .source
    .actual_subscribe(BufferWithCountObserver{
      observer: $observer,
      buffer: vec![],
      count:$self.count
    })
  }
  where
    S: @ctx::Observable,
    S::Item: @ctx::shared_only(Send + Sync + 'static) @ctx::local_only('o)
}

#[derive(Clone)]
pub struct BufferWithCountObserver<O, Item> {
  observer: O,
  buffer: Vec<Item>,
  count: usize,
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

#[derive(Clone)]
pub struct BufferWithTimeObserver<O, B, U> {
  observer: O,
  buffer: B,
  handler: U,
}

macro_rules! new_buffer_time_observer {
  ($observer:ident, $scheduler: expr, $time: expr,  $ctx: ident) => {{
    let observer = $ctx::Rc::own($observer);
    let mut observer_c = observer.clone();
    let buffer = $ctx::Rc::own(vec![]);
    let buffer_c = buffer.clone();

    let handler = $scheduler.schedule_repeating(
      move |_| {
        let b = &mut *buffer_c.rc_deref_mut();
        if !b.is_empty() {
          observer_c.next(std::mem::take(b));
        }
      },
      $time,
      None,
    );
    let handler = $ctx::Rc::own(handler);
    BufferWithTimeObserver {
      observer,
      buffer,
      handler,
    }
  }};
}

impl<S: Observable, SD> Observable for BufferWithTimeOp<S, SD> {
  type Item = Vec<S::Item>;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<Source, SD> BufferWithTimeOp<Source, SD>;
  type Unsub = TimeSubscription<@ctx::Rc<SpawnHandle>, Source::Unsub>;
  macro method($self:ident, $observer: ident, $ctx: ident) {
    let observer = new_buffer_time_observer!(
      $observer, $self.scheduler, $self.time, $ctx
    );
    let handler = observer.handler.clone();
    let subscription = $self.source.actual_subscribe(observer);
    TimeSubscription{ handler, subscription }
  }
  where
    @ctx::local_only('o: 'static,)
    Source: @ctx::Observable,
    Source::Item: @ctx::shared_only(Send + Sync +) 'static,
    SD: @ctx::Scheduler + 'static,
}

pub struct TimeSubscription<H, U> {
  handler: H,
  subscription: U,
}

impl<U: SubscriptionLike, H: SubscriptionLike> SubscriptionLike
  for TimeSubscription<H, U>
{
  fn unsubscribe(&mut self) {
    self.handler.unsubscribe();
    self.subscription.unsubscribe();
  }

  fn is_closed(&self) -> bool { self.handler.is_closed() }
}

impl<O, B, U, Item, Err> Observer for BufferWithTimeObserver<O, B, U>
where
  O: Observer<Item = Vec<Item>, Err = Err>,
  B: RcDerefMut + 'static,
  U: SubscriptionLike,
  for<'r> B::Target<'r>: DerefMut<Target = O::Item>,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    if !self.handler.is_closed() {
      self.buffer.rc_deref_mut().push(value);
    }
  }

  fn error(&mut self, err: Self::Err) {
    if !self.handler.is_closed() {
      self.handler.unsubscribe();
      self.observer.error(err);
    }
  }

  fn complete(&mut self) {
    if !self.handler.is_closed() {
      let buffer = std::mem::take(&mut *self.buffer.rc_deref_mut());
      if !buffer.is_empty() {
        self.observer.next(buffer);
      }

      self.handler.unsubscribe();
      self.observer.complete();
    }
  }
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerOp<Source, Scheduler> {
  pub(crate) source: Source,
  pub(crate) count: usize,
  pub(crate) time: Duration,
  pub(crate) scheduler: Scheduler,
}

impl<S: Observable, SD> Observable for BufferWithCountOrTimerOp<S, SD> {
  type Item = Vec<S::Item>;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<Source, SD> BufferWithCountOrTimerOp<Source, SD>;
  type Unsub = TimeSubscription<@ctx::Rc<SpawnHandle>, Source::Unsub>;
  macro method($self:ident, $observer: ident, $ctx: ident) {
    let observer = new_buffer_time_observer!(
      $observer,
      $self.scheduler,
      $self.time,
      $ctx
    );
    let handler = observer.handler.clone();
    let subscription = $self
      .source
      .actual_subscribe(BufferWithCountOrTimerObserver {
        observer,
        count: $self.count,
      });
      TimeSubscription{ handler, subscription }
  }
  where
    @ctx::local_only('o: 'static,)
    Source: @ctx::Observable,
    // Source::Unsub: 'static,
    Source::Item: @ctx::shared_only(Send + Sync +) 'static,
    SD: @ctx::Scheduler + 'static,
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerObserver<O, B, U> {
  observer: BufferWithTimeObserver<O, B, U>,
  count: usize,
}

impl<O, B, U, Item> Observer for BufferWithCountOrTimerObserver<O, B, U>
where
  O: Observer<Item = Vec<Item>>,
  B: RcDerefMut + 'static,
  for<'r> B::Target<'r>: DerefMut<Target = O::Item>,
  U: SubscriptionLike,
{
  type Item = Item;
  type Err = O::Err;

  fn next(&mut self, value: Self::Item) {
    if !self.observer.handler.is_closed() {
      self.observer.next(value);

      let mut rc_buffer = self.observer.buffer.rc_deref_mut();
      if rc_buffer.len() >= self.count {
        let buffer = std::mem::take(&mut *rc_buffer);
        self.observer.observer.next(buffer);
      }
    }
  }

  #[inline]
  fn error(&mut self, err: Self::Err) { self.observer.error(err); }

  #[inline]
  fn complete(&mut self) { self.observer.complete() }
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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

    observable::create(|subscriber| {
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
