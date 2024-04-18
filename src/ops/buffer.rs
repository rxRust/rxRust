use crate::{
  prelude::*,
  rc::{MutArc, RcDerefMut},
};
use std::vec;

#[derive(Clone)]
pub struct BufferOp<S, N> {
  pub(crate) source: S,
  pub(crate) closing_notifier: N,
}

impl<Item, Err, O, S, N> Observable<Vec<Item>, Err, O> for BufferOp<S, N>
where
  S: Observable<Item, Err, RcBufferObserver<O, Item>>,
  O: Observer<Vec<Item>, Err>,
  N: Observable<(), Err, NotifierObserver<O, Item>>,
{
  type Unsub = ZipSubscription<S::Unsub, N::Unsub>;
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let observer = MutArc::own(Some(BufferObserver { observer, data: vec![] }));
    ZipSubscription::new(
      self.source.actual_subscribe(observer.clone()),
      self
        .closing_notifier
        .actual_subscribe(NotifierObserver(observer)),
    )
  }
}

impl<Item, Err, S, N> ObservableExt<Vec<Item>, Err> for BufferOp<S, N>
where
  S: ObservableExt<Item, Err>,
  N: ObservableExt<(), Err>,
{
}

#[derive(Clone)]
pub struct BufferWithCountOp<S> {
  pub(crate) source: S,
  pub(crate) count: usize,
}

impl<Item, Err, O, S> Observable<Vec<Item>, Err, O> for BufferWithCountOp<S>
where
  S: Observable<Item, Err, BufferWithCountObserver<O, Item>>,
  O: Observer<Vec<Item>, Err>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(BufferWithCountObserver {
      buffer: BufferObserver { observer, data: vec![] },
      count: self.count,
    })
  }
}

impl<Item, Err, S> ObservableExt<Vec<Item>, Err> for BufferWithCountOp<S> where
  S: ObservableExt<Item, Err>
{
}

#[derive(Clone)]
pub struct BufferObserver<O, Item> {
  observer: O,
  data: Vec<Item>,
}

pub struct NotifierObserver<O, Item>(RcBufferObserver<O, Item>);

impl<O, Item, Err> Observer<(), Err> for NotifierObserver<O, Item>
where
  O: Observer<Vec<Item>, Err>,
{
  fn next(&mut self, _value: ()) {
    if let Some(v) = self.0.rc_deref_mut().as_mut() {
      v.emit()
    }
  }

  #[inline]
  fn complete(self) {
    self.0.complete()
  }

  #[inline]
  fn error(self, err: Err) {
    self.0.error(err)
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.0.is_finished()
  }
}

impl<O, Item, Err> Observer<Item, Err> for BufferObserver<O, Item>
where
  O: Observer<Vec<Item>, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.data.push(value);
  }

  fn complete(mut self) {
    self.emit();
    self.observer.complete();
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

impl<O, Item> BufferObserver<O, Item> {
  fn emit<Err>(&mut self)
  where
    O: Observer<Vec<Item>, Err>,
  {
    if !self.data.is_empty() {
      let buffer = std::mem::take(&mut self.data);
      self.observer.next(buffer);
    }
  }
}

#[derive(Clone)]
pub struct BufferWithCountObserver<O, Item> {
  buffer: BufferObserver<O, Item>,
  count: usize,
}

impl<O, Item, Err> Observer<Item, Err> for BufferWithCountObserver<O, Item>
where
  O: Observer<Vec<Item>, Err>,
{
  fn next(&mut self, value: Item) {
    self.buffer.next(value);

    if self.buffer.data.len() >= self.count {
      self.buffer.emit()
    }
  }

  #[inline]
  fn complete(self) {
    self.buffer.complete()
  }

  #[inline]
  fn error(self, err: Err) {
    self.buffer.error(err)
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.buffer.is_finished()
  }
}

#[derive(Clone)]
pub struct BufferWithTimeOp<Source, Scheduler> {
  pub(crate) source: Source,
  pub(crate) time: Duration,
  pub(crate) scheduler: Scheduler,
}

type RcBufferObserver<O, Item> = MutArc<Option<BufferObserver<O, Item>>>;

fn emit_buffer<O, Item, Err>(
  observer: &mut RcBufferObserver<O, Item>,
  _seq: usize,
) -> bool
where
  O: Observer<Vec<Item>, Err>,
{
  if !observer.is_finished() {
    if let Some(v) = observer.rc_deref_mut().as_mut() {
      v.emit()
    }
    true
  } else {
    false
  }
}

impl<S, Item, Err, O, SD> Observable<Vec<Item>, Err, O>
  for BufferWithTimeOp<S, SD>
where
  O: Observer<Vec<Item>, Err>,
  S: Observable<Item, Err, RcBufferObserver<O, Item>>,
  SD: Scheduler<RepeatTask<RcBufferObserver<O, Item>>>,
{
  type Unsub = ZipSubscription<TaskHandle<NormalReturn<()>>, S::Unsub>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, time, scheduler } = self;
    let observer = BufferObserver { observer, data: vec![] };
    let observer = MutArc::own(Some(observer));

    let handler = scheduler
      .schedule(RepeatTask::new(time, emit_buffer, observer.clone()), None);

    let subscription = source.actual_subscribe(observer);
    ZipSubscription::new(handler, subscription)
  }
}

impl<Item, Err, S, SD> ObservableExt<Vec<Item>, Err> for BufferWithTimeOp<S, SD> where
  S: ObservableExt<Item, Err>
{
}

#[derive(Clone)]
pub struct BufferWithCountOrTimerOp<Source, Scheduler> {
  pub(crate) source: Source,
  pub(crate) count: usize,
  pub(crate) time: Duration,
  pub(crate) scheduler: Scheduler,
}

type RcBufferWitchCountObserver<O, Item> =
  MutArc<Option<BufferWithCountObserver<O, Item>>>;

fn emit_count_buffer<O, Item, Err>(
  observer: &mut RcBufferWitchCountObserver<O, Item>,
  _seq: usize,
) -> bool
where
  O: Observer<Vec<Item>, Err>,
{
  if !observer.is_finished() {
    if let Some(v) = observer.rc_deref_mut().as_mut() {
      v.buffer.emit()
    }
    true
  } else {
    false
  }
}

impl<S, SD, Item, Err, O> Observable<Vec<Item>, Err, O>
  for BufferWithCountOrTimerOp<S, SD>
where
  O: Observer<Vec<Item>, Err>,
  S: Observable<Item, Err, RcBufferWitchCountObserver<O, Item>>,
  SD: Scheduler<RepeatTask<RcBufferWitchCountObserver<O, Item>>>,
{
  type Unsub = ZipSubscription<TaskHandle<NormalReturn<()>>, S::Unsub>;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let Self { source, time, scheduler, count } = self;

    let observer = BufferWithCountObserver {
      buffer: BufferObserver { observer, data: vec![] },
      count,
    };
    let observer = MutArc::own(Some(observer));

    let handler = scheduler.schedule(
      RepeatTask::new(time, emit_count_buffer, observer.clone()),
      None,
    );

    let subscription = source.actual_subscribe(observer);
    ZipSubscription::new(handler, subscription)
  }
}

impl<Item, Err, S, SD> ObservableExt<Vec<Item>, Err>
  for BufferWithCountOrTimerOp<S, SD>
where
  S: ObservableExt<Item, Err>,
{
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use futures::executor::LocalPool;
  use std::cell::RefCell;
  use std::convert::Infallible;
  use std::rc::Rc;
  use std::sync::atomic::{AtomicBool, Ordering};

  #[test]
  fn it_shall_buffer() {
    let mut closing_notifier = Subject::<(), Infallible>::default();
    let mut source = Subject::<i32, Infallible>::default();
    let expected = vec![vec![0, 1], vec![2]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();
    source
      .clone()
      .buffer(closing_notifier.clone())
      .subscribe(move |vec| actual_c.borrow_mut().push(vec));
    source.next(0);
    source.next(1);
    closing_notifier.next(());
    source.next(2);
    closing_notifier.next(());
    source.next(3);
    assert_eq!(expected, *actual.borrow());
  }

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
  fn it_shall_emit_buffer_on_completed() {
    let expected = vec![vec![0, 1], vec![2, 3], vec![4]];
    let mut actual = vec![];

    let is_completed = Rc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.complete();
    })
    .buffer_with_count(2)
    .on_complete(move || is_completed_c.store(true, Ordering::Relaxed))
    .subscribe(|vec| actual.push(vec));

    assert_eq!(expected, actual);
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_discard_buffer_on_error() {
    let expected = vec![vec![0, 1], vec![2, 3]];
    let mut actual = vec![];
    let mut err_called = false;

    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.next(3);
      subscriber.next(4);
      subscriber.error(());
    })
    .buffer_with_count(2)
    .on_error(|_| err_called = true)
    .subscribe(|vec| actual.push(vec));

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

    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.error(());
    })
    .buffer_with_time(Duration::from_millis(500), local.spawner())
    .on_error(|_| {})
    .subscribe(|_| {});

    // if this call blocks execution, the observer's handle has not been
    // unsubscribed
    local.run();
  }

  #[test]
  fn it_shall_buffer_with_time_with_observable_ext() {
    let mut local = LocalPool::new();

    let expected = vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();

    observable::from_iter(0..10)
      .buffer_with_time(Duration::from_millis(500), local.spawner())
      .filter(move |_vec: &Vec<i32>| true)
      .subscribe(move |vec| actual_c.borrow_mut().push(vec));

    local.run();

    // this can't be really tested as local scheduler runs on a single thread
    assert_eq!(expected, *actual.borrow());
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

    observable::create(|mut subscriber: Subscriber<_>| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.error(());
    })
    .buffer_with_count_and_time(2, Duration::from_millis(500), local.spawner())
    .on_error(move |_| error_called_c.store(true, Ordering::Relaxed))
    .subscribe(move |vec| actual_c.borrow_mut().push(vec));

    local.run();

    assert_eq!(expected, *actual.borrow());
    assert!(error_called.load(Ordering::Relaxed));
  }
}
