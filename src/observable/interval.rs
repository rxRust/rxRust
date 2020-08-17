use crate::prelude::*;
use futures::prelude::*;
use futures_timer::Delay;
use std::{
  future::Future,
  time::{Duration, Instant},
};

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
pub fn interval<S>(
  dur: Duration,
  scheduler: S,
) -> ObservableBase<IntervalEmitter<S>> {
  ObservableBase::new(IntervalEmitter {
    dur,
    at: Instant::now() + dur,
    scheduler,
  })
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
pub fn interval_at<S>(
  at: Instant,
  dur: Duration,
  scheduler: S,
) -> ObservableBase<IntervalEmitter<S>> {
  ObservableBase::new(IntervalEmitter { dur, at, scheduler })
}

#[derive(Clone)]
pub struct IntervalEmitter<S> {
  scheduler: S,
  dur: Duration,
  at: Instant,
}

impl<S> IntervalEmitter<S> {
  fn interval_future(
    &self,
    mut observer: impl Observer<usize, ()>,
  ) -> impl Future<Output = ()> {
    let mut number = 0;
    let now = Instant::now();
    let delay = if self.at > now {
      self.at - now
    } else {
      Duration::from_micros(0)
    };
    let dur = self.dur;

    Delay::new(delay).then(move |_| {
      observer.next(number);
      number += 1;
      async_std::stream::interval(dur).for_each(move |_| {
        observer.next(number);
        number += 1;
        future::ready(())
      })
    })
  }
}

impl<S> Emitter for IntervalEmitter<S> {
  type Item = usize;
  type Err = ();
}

impl<S: SharedScheduler + 'static> SharedEmitter for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
  {
    let Subscriber {
      observer,
      mut subscription,
    } = subscriber;
    let f = self.interval_future(observer);
    self.scheduler.spawn(f, &mut subscription);
  }
}

impl<S: LocalScheduler + 'static> LocalEmitter<'static> for IntervalEmitter<S> {
  fn emit<O>(self, subscriber: Subscriber<O, LocalSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + 'static,
  {
    let Subscriber {
      observer,
      mut subscription,
    } = subscriber;
    let f = self.interval_future(observer);
    self.scheduler.spawn(f, &mut subscription);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
  };

  #[test]
  fn shared() {
    let seconds = Arc::new(Mutex::new(0));
    let c_seconds = seconds.clone();
    let pool = ThreadPool::new().unwrap();

    interval(Duration::from_millis(10), pool)
      .to_shared()
      .subscribe(move |_| {
        *seconds.lock().unwrap() += 1;
      });

    std::thread::sleep(Duration::from_millis(55));
    assert_eq!(*c_seconds.lock().unwrap(), 5);
  }

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let millis = Rc::new(RefCell::new(0));
    let c_millis = millis.clone();
    interval(Duration::from_millis(1), local.spawner()).subscribe(move |_| {
      *c_millis.borrow_mut() += 1;
    });

    local.run_until(futures_timer::Delay::new(Duration::from_millis(5)));
    assert_eq!(*millis.borrow(), 5);
  }
}
