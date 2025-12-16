//! BufferTime operator implementation
//!
//! Collects items emitted by the source observable into buffers based on time
//! windows. When the time window expires, the buffer is emitted and a new
//! window starts. Optionally, a max buffer size can be specified to emit
//! the buffer early when the count is reached.

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskHandle, TaskState},
  subscription::{SourceWithHandle, Subscription},
};

// ==================== BufferTime ====================

/// Collects items for a time window, emitting the buffer when the window
/// expires or when the max buffer size is reached (if specified).
///
/// This aligns with RxJS's `bufferTime(bufferTimeSpan, bufferCreationInterval?,
/// maxBufferSize?)`.
#[derive(Clone)]
pub struct BufferTime<S, Sch> {
  pub source: S,
  pub duration: Duration,
  pub max_buffer_size: Option<usize>,
  pub scheduler: Sch,
}

impl<S, Sch> ObservableType for BufferTime<S, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = Vec<S::Item<'a>>
  where
    Self: 'a;
  type Err = S::Err;
}

// ==================== State ====================

/// Shared state for buffer with time.
pub struct BufferTimeState<O, Item> {
  observer: Option<O>,
  buffer: Vec<Item>,
  max_buffer_size: Option<usize>,
  completed: bool,
}

impl<O, Item> BufferTimeState<O, Item> {
  fn new(observer: O, max_buffer_size: Option<usize>) -> Self {
    let buffer = match max_buffer_size {
      Some(size) => Vec::with_capacity(size),
      None => Vec::new(),
    };
    Self { observer: Some(observer), buffer, max_buffer_size, completed: false }
  }

  fn flush_buffer(&mut self) -> Vec<Item> {
    let capacity = self.max_buffer_size.unwrap_or(0);
    std::mem::replace(&mut self.buffer, Vec::with_capacity(capacity))
  }
}

// ==================== Observer ====================

/// Observer for buffer_time.
pub struct BufferTimeObserver<R> {
  state: R,
}

impl<R, O, Item, Err> Observer<Item, Err> for BufferTimeObserver<R>
where
  R: RcDerefMut<Target = BufferTimeState<O, Item>>,
  O: Observer<Vec<Item>, Err>,
{
  fn next(&mut self, v: Item) {
    let mut state = self.state.rc_deref_mut();
    state.buffer.push(v);

    // Check if max buffer size is reached
    if let Some(max_size) = state.max_buffer_size
      && state.buffer.len() >= max_size
    {
      let buffer = state.flush_buffer();
      if let Some(ref mut observer) = state.observer {
        observer.next(buffer);
      }
    }
  }

  fn error(self, e: Err) {
    let mut state = self.state.rc_deref_mut();
    state.completed = true;
    if let Some(observer) = state.observer.take() {
      observer.error(e);
    }
  }

  fn complete(self) {
    let mut state = self.state.rc_deref_mut();
    state.completed = true;
    if let Some(mut observer) = state.observer.take() {
      if !state.buffer.is_empty() {
        observer.next(state.flush_buffer());
      }
      observer.complete();
    }
  }

  fn is_closed(&self) -> bool { self.state.rc_deref_mut().observer.is_none() }
}

// ==================== Task Handler ====================

/// Handler for time-based buffer emission.
fn buffer_time_handler<O, Item, Err, R>((task_state, duration): &mut (R, Duration)) -> TaskState
where
  R: RcDerefMut<Target = BufferTimeState<O, Item>>,
  O: Observer<Vec<Item>, Err>,
{
  let mut state = task_state.rc_deref_mut();
  if state.completed || state.observer.is_none() {
    return TaskState::Finished;
  }

  if !state.buffer.is_empty() {
    let buffer = state.flush_buffer();
    if let Some(ref mut observer) = state.observer {
      observer.next(buffer);
    }
  }

  TaskState::Sleeping(*duration)
}

// ==================== Subscription ====================

pub type BufferTimeSubscription<U, H> = SourceWithHandle<U, H>;

// ==================== CoreObservable ====================

impl<S, Sch, C, Unsub> CoreObservable<C> for BufferTime<S, Sch>
where
  C: Context,
  S: ObservableType + 'static,
  C::Inner: for<'a> Observer<Vec<<S as ObservableType>::Item<'a>>, S::Err>,
  S: for<'a> CoreObservable<
      C::With<
        BufferTimeObserver<C::RcMut<BufferTimeState<C::Inner, <S as ObservableType>::Item<'a>>>>,
      >,
      Unsub = Unsub,
    >,
  Sch: for<'a> Scheduler<
    Task<(C::RcMut<BufferTimeState<C::Inner, <S as ObservableType>::Item<'a>>>, Duration)>,
  >,
  Unsub: Subscription,
{
  type Unsub = BufferTimeSubscription<Unsub, TaskHandle>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let BufferTime { source, duration, max_buffer_size, scheduler } = self;

    let state = C::RcMut::from(BufferTimeState::new(context.into_inner(), max_buffer_size));

    // Schedule repeating timer for buffer emission
    let task = Task::new((state.clone(), duration), buffer_time_handler);
    let handle = scheduler.schedule(task, Some(duration));

    let wrapped = C::lift(BufferTimeObserver { state });
    let source_sub = source.subscribe(wrapped);

    SourceWithHandle::new(source_sub, handle)
  }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc, time::Duration};

  use crate::prelude::*;

  #[rxrust_macro::test]
  fn test_buffer_time() {
    use std::convert::Infallible;

    use crate::{context::TestCtx, scheduler::test_scheduler::TestScheduler};

    TestScheduler::init();

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();

    let _sub = subject
      .clone()
      .buffer_time(Duration::from_millis(100))
      .subscribe(move |v| result_clone.borrow_mut().push(v));

    subject.next(1);
    subject.next(2);

    // No buffer emitted yet
    assert!(result.borrow().is_empty());

    // Advance time past the buffer window
    TestScheduler::advance_by(Duration::from_millis(100));

    // Buffer should be emitted
    assert_eq!(*result.borrow(), vec![vec![1, 2]]);

    subject.next(3);
    subject.next(4);
    subject.next(5);

    TestScheduler::advance_by(Duration::from_millis(100));

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3, 4, 5]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_time_max_count_first() {
    use std::convert::Infallible;

    use crate::{context::TestCtx, scheduler::test_scheduler::TestScheduler};

    TestScheduler::init();

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();

    let _sub = subject
      .clone()
      .buffer_time_max(Duration::from_millis(100), 2)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Emit 2 values quickly - should trigger count-based emit
    subject.next(1);
    subject.next(2);

    // Buffer should be emitted due to count
    assert_eq!(*result.borrow(), vec![vec![1, 2]]);

    // Emit one more value
    subject.next(3);

    // Advance time to trigger time-based emit
    TestScheduler::advance_by(Duration::from_millis(100));

    assert_eq!(*result.borrow(), vec![vec![1, 2], vec![3]]);
  }

  #[rxrust_macro::test]
  fn test_buffer_time_max_time_first() {
    use std::convert::Infallible;

    use crate::{context::TestCtx, scheduler::test_scheduler::TestScheduler};

    TestScheduler::init();

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let mut subject = TestCtx::subject::<i32, Infallible>();

    let _sub = subject
      .clone()
      .buffer_time_max(Duration::from_millis(50), 5)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    // Emit only 2 values (less than count of 5)
    subject.next(1);
    subject.next(2);

    // No buffer emitted yet (count not reached)
    assert!(result.borrow().is_empty());

    // Advance time to trigger time-based emit
    TestScheduler::advance_by(Duration::from_millis(50));

    // Buffer should be emitted due to time
    assert_eq!(*result.borrow(), vec![vec![1, 2]]);
  }
}
