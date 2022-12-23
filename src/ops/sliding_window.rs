use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
use std::ops::DerefMut;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SlidingWindowWithTimeFunctionOperation<
  Source,
  TimeFunction,
  Scheduler,
> {
  pub(crate) source: Source,
  pub(crate) window_size: Duration,
  pub(crate) scanning_interval: Duration,
  pub(crate) time_function: TimeFunction,
  pub(crate) scheduler: Scheduler,
}

impl<Source, TimeFunction, Scheduler> Observable
  for SlidingWindowWithTimeFunctionOperation<Source, TimeFunction, Scheduler>
where
  Source: Observable,
  TimeFunction: Fn(Source::Item) -> Duration,
{
  type Item = Vec<Source::Item>;
  type Err = Source::Err;
}

#[derive(Clone)]
pub struct SlidingWindowWithTimeFunctionObserver<Observer, Buffer, Handler> {
  observer: Observer,
  buffer: Buffer,
  handler: Handler,
}

impl<Obs, Buffer, Handler, Item, Err> Observer
  for SlidingWindowWithTimeFunctionObserver<Obs, Buffer, Handler>
where
  Obs: Observer<Item = Vec<Item>, Err = Err>,
  Buffer: RcDerefMut + 'static,
  Handler: SubscriptionLike,
  for<'r> Buffer::Target<'r>: DerefMut<Target = Obs::Item>,
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

macro_rules! new_sliding_window_observer {
  ($observer:ident, $scheduler: expr, $window_size: expr, $scanning_interval: expr, $time_function: expr,  $ctx: ident) => {{
    let observer = $ctx::Rc::own($observer);
    let mut observer_c = observer.clone();
    let buffer: $ctx::Rc<Vec<Source::Item>> = $ctx::Rc::own(vec![]);
    let buffer_c = buffer.clone();

    let handler = $scheduler.schedule_repeating(
      move |_| {
        let buffer = &mut *buffer_c.rc_deref_mut();
        if !buffer.is_empty() {
          buffer.drain_filter(|message| {
            $time_function(*message) + $window_size < get_now_duration()
          });
          let copied_buffer = buffer.iter().map(|message| *message).collect();
          observer_c.next(copied_buffer);
        }
      },
      $scanning_interval,
      None,
    );
    let handler = $ctx::Rc::own(handler);
    SlidingWindowWithTimeFunctionObserver {
      observer,
      buffer,
      handler,
    }
  }};
}

impl_local_shared_both! {
    impl<Source, TimeFunction, Scheduler> SlidingWindowWithTimeFunctionOperation<Source, TimeFunction, Scheduler>;
    type Unsub =  TimeSubscription<@ctx::Rc<SpawnHandle>, Source::Unsub>;
    macro method($self: ident, $observer: ident, $ctx: ident) {
        let observer = new_sliding_window_observer!(
            $observer, $self.scheduler, $self.window_size, $self.scanning_interval, $self.time_function, $ctx
        );
        let handler = observer.handler.clone();
        let subscription = $self.source.actual_subscribe(observer);
        TimeSubscription{handler, subscription}
    }
    where
        @ctx::local_only('o: 'static,)
        Source: @ctx::Observable,
        Source::Item: @ctx::shared_only(Send + Sync  +) 'static + Clone + Copy,
        Scheduler: @ctx::Scheduler + 'static,
        TimeFunction: Fn(Source::Item) -> Duration + @ctx::shared_only(Send + Sync +) 'static + Copy,
}

pub fn get_now_duration() -> Duration {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Could not get epoch seconds")
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

  fn is_closed(&self) -> bool {
    self.handler.is_closed()
  }
}

#[cfg(test)]
mod tests {
  use crate::observable;
  use crate::observable::Observable;
  use crate::ops::sliding_window::get_now_duration;
  use crate::prelude::{SharedObservable, SubscribeAll, SubscribeNext};
  use futures::executor::LocalPool;
  use futures::executor::ThreadPool;
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  #[test]
  fn it_shall_make_a_window_local() {
    let mut local = LocalPool::new();
    let expected = vec![vec![0, 1, 2]];
    let actual = Rc::new(RefCell::new(vec![]));
    let actual_c = actual.clone();

    observable::from_iter(0..3)
      .map(|i| (i, Duration::from_secs(1)))
      .sliding_window(
        Duration::from_secs(1),
        Duration::from_millis(250),
        |(_, duration)| duration,
        local.spawner(),
      )
      .map(|window| window.iter().map(|(i, _)| *i).collect::<Vec<i32>>())
      .subscribe(move |vec| actual_c.borrow_mut().push(vec));

    local.run();

    // this can't be really tested as local scheduler runs on a single thread
    assert_eq!(expected, *actual.borrow());
  }

  #[test]
  fn it_shall_not_block_on_error_local() {
    let mut local = LocalPool::new();

    observable::create(|subscriber| {
      subscriber.next(0);
      subscriber.next(1);
      subscriber.next(2);
      subscriber.error(());
    })
    .map(|i| (i, Duration::from_secs(1)))
    .sliding_window(
      Duration::from_secs(1),
      Duration::from_millis(250),
      |(_, duration)| duration,
      local.spawner(),
    )
    .subscribe(|_| {});

    // if this call blocks execution, the observer's handle has not been
    // unsubscribed
    local.run();
  }

  #[test]
  fn it_shall_make_a_window_shared() {
    let pool = ThreadPool::new().unwrap();

    let expected = vec![
      vec![0],
      vec![0, 1],
      vec![0, 1],
      vec![1, 2],
      vec![1, 2],
      vec![2, 3],
      vec![2, 3],
      vec![2, 3],
    ];
    let actual = Arc::new(Mutex::new(vec![]));
    let actual_c = actual.clone();

    let is_completed = Arc::new(AtomicBool::new(false));
    let is_completed_c = is_completed.clone();

    observable::create(|subscriber| {
      let sleep = Duration::from_millis(100);
      subscriber.next(0);
      std::thread::sleep(sleep);
      subscriber.next(1);
      std::thread::sleep(sleep);
      subscriber.next(2);
      std::thread::sleep(sleep);
      subscriber.next(3);
      std::thread::sleep(sleep);
      subscriber.complete();
    })
    .map(|i| (i, get_now_duration()))
    .sliding_window(
      Duration::from_millis(210),
      Duration::from_millis(53),
      |(_, duration)| duration,
      pool,
    )
    .map(|window| window.iter().map(|(i, _)| *i).collect::<Vec<i32>>())
    .into_shared()
    .subscribe_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      |()| {},
      move || is_completed_c.store(true, Ordering::Relaxed),
    );

    std::thread::sleep(Duration::from_millis(450));
    assert_eq!(expected, *actual.lock().unwrap());
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[test]
  fn it_shall_not_emit_window_on_error() {
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
    .map(|i| (i, get_now_duration()))
    .sliding_window(
      Duration::from_millis(210),
      Duration::from_millis(52),
      |(_, duration)| duration,
      pool,
    )
    .map(|window| window.iter().map(|(i, _)| *i).collect::<Vec<i32>>())
    .into_shared()
    .subscribe_all(
      move |vec| {
        let mut a = actual_c.lock().unwrap();
        (*a).push(vec);
      },
      move |_| error_called_c.store(true, Ordering::Relaxed),
      || {},
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(expected, *actual.lock().unwrap());
    assert!(error_called.load(Ordering::Relaxed));
  }
}
