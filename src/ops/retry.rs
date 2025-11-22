use crate::{
  observer::{BoxObserver, BoxObserverThreads},
  prelude::*,
  rc::{MutArc, MutRc, RcDerefMut},
};

pub struct RetryOp<S, Config, SD> {
  source: S,
  config: Config,
  scheduler: SD,
  retry_count: usize,
}

pub struct RetryThreadsOp<S, Config, SD> {
  source: S,
  config: Config,
  scheduler: SD,
  retry_count: usize,
}

pub trait RetryConfig<Err>
where
  Self: Clone,
{
  fn count(&self) -> Option<usize>;
  fn delay(&self, err: Err, retry_count: usize) -> Option<Duration>;
  fn reset_on_success(&self) -> bool;
}

pub struct SimpleRetryConfig {
  count: Option<usize>,
  delay: Option<Duration>,
  reset_on_success: bool,
}

impl Clone for SimpleRetryConfig {
  fn clone(&self) -> Self {
    Self {
      count: self.count,
      delay: self.delay,
      reset_on_success: self.reset_on_success,
    }
  }
}

impl SimpleRetryConfig {
  pub fn new() -> Self {
    Self {
      count: None,
      delay: None,
      reset_on_success: false,
    }
  }

  pub fn count(mut self, count: usize) -> Self {
    self.count = Some(count);
    self
  }

  pub fn delay(mut self, delay: Duration) -> Self {
    self.delay = Some(delay);
    self
  }

  pub fn reset_on_success(mut self) -> Self {
    self.reset_on_success = true;
    self
  }
}

impl<Err> RetryConfig<Err> for SimpleRetryConfig {
  fn count(&self) -> Option<usize> {
    self.count
  }

  fn delay(&self, _err: Err, _retry_count: usize) -> Option<Duration> {
    self.delay
  }

  fn reset_on_success(&self) -> bool {
    self.reset_on_success
  }
}

impl Default for SimpleRetryConfig {
  fn default() -> Self {
    Self::new()
  }
}

macro_rules! impl_retry_op {
  ($name: ident, $rc: ident) => {
    impl<S, Config, SD> $name<S, Config, SD> {
      #[inline]
      pub fn new(source: S, config: Config, scheduler: SD) -> Self {
        Self {
          source,
          config,
          scheduler,
          retry_count: 0,
        }
      }
    }
  };
}

impl_retry_op!(RetryOp, MutRc);
impl_retry_op!(RetryThreadsOp, MutArc);

macro_rules! impl_retry_observable {
  (
    $retry_op: ident,
    $retry_observer: ident,
    $rc: ident,
    $box_observer: ty,
    $box_subscription: ty,
    ($($threads_bound:tt)?),
    $subscribe: ident,
    $subscribe_task: ident
  ) => {
    impl<S, Item, Err, O, Config, SD> Observable<Item, Err, O>
      for $retry_op<S, Config, SD>
    where
      S: Observable<Item, Err, $box_observer> + Clone + 'static $(+ $threads_bound)?,
      S::Unsub: 'static $(+ $threads_bound)?,
      O: Observer<Item, Err> + 'static $(+ $threads_bound)?,
      Err: 'static,
      Config: RetryConfig<Err> + 'static $(+ $threads_bound)?,
      SD: Scheduler<
        OnceTask<
          (
            $retry_op<S, Config, SD>,
            O,
            $rc<Option<$box_subscription>>,
          ),
          NormalReturn<()>,
        >,
      > + 'static $(+ $threads_bound)?,
    {
      type Unsub = $rc<Option<$box_subscription>>;

      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        let subscription = $rc::own(None);

        $subscribe(self, observer, subscription.clone(), None);

        subscription
      }
    }

    impl<S, Item, Err, Config, SD> ObservableExt<Item, Err>
      for $retry_op<S, Config, SD>
    where
      S: ObservableExt<Item, Err>,
    {
    }

    fn $subscribe<S, O, Item, Err, Config, SD>(
      op: $retry_op<S, Config, SD>,
      observer: O,
      subscription: $rc<Option<$box_subscription>>,
      delay: Option<Duration>,
    ) where
      S: Observable<Item, Err, $box_observer> + Clone + 'static $(+ $threads_bound)?,
      S::Unsub: 'static $(+ $threads_bound)?,
      O: Observer<Item, Err> + 'static $(+ $threads_bound)?,
      Err: 'static,
      Config: RetryConfig<Err> + 'static $(+ $threads_bound)?,
      SD: Scheduler<
        OnceTask<
          (
            $retry_op<S, Config, SD>,
            O,
            $rc<Option<$box_subscription>>,
          ),
          NormalReturn<()>,
        >,
      > + 'static $(+ $threads_bound)?,
    {
      op.scheduler.clone().schedule(
        OnceTask::new($subscribe_task, (op, observer, subscription)),
        delay,
      );
    }

    fn $subscribe_task<S, O, Item, Err, Config, SD>(
      (op, observer, subscription): (
        $retry_op<S, Config, SD>,
        O,
        $rc<Option<$box_subscription>>,
      ),
    ) -> NormalReturn<()>
    where
      S: Observable<Item, Err, $box_observer> + Clone + 'static $(+ $threads_bound)?,
      S::Unsub: 'static $(+ $threads_bound)?,
      O: Observer<Item, Err> + 'static $(+ $threads_bound)?,
      Err: 'static,
      Config: RetryConfig<Err> + 'static $(+ $threads_bound)?,
      SD: Scheduler<
        OnceTask<
          (
            $retry_op<S, Config, SD>,
            O,
            $rc<Option<$box_subscription>>,
          ),
          NormalReturn<()>,
        >,
      > + 'static $(+ $threads_bound)?,
    {
      let unsub =
        op.source
          .clone()
          .actual_subscribe(<$box_observer>::new($retry_observer {
            op,
            observer,
            subscription: subscription.clone(),
            _hint: TypeHint::default(),
          }));

      subscription
        .rc_deref_mut()
        .replace(<$box_subscription>::new(unsub));

      NormalReturn::new(())
    }
  };
}

impl_retry_observable!(
  RetryOp,
  RetryObserver,
  MutRc,
  BoxObserver<'static, Item, Err>,
  BoxSubscription<'static>,
  (),
  subscribe,
  subscribe_task
);
impl_retry_observable!(
  RetryThreadsOp,
  RetryThreadsObserver,
  MutArc,
  BoxObserverThreads<Item, Err>,
  BoxSubscriptionThreads,
  (Send),
  subscribe_threads,
  subscribe_task_threads
);

pub struct RetryObserver<S, O, Err, Config, SD> {
  op: RetryOp<S, Config, SD>,
  observer: O,
  subscription: MutRc<Option<BoxSubscription<'static>>>,
  _hint: TypeHint<Err>,
}

pub struct RetryThreadsObserver<S, O, Err, Config, SD> {
  op: RetryThreadsOp<S, Config, SD>,
  observer: O,
  subscription: MutArc<Option<BoxSubscriptionThreads>>,
  _hint: TypeHint<Err>,
}

macro_rules! impl_retry_observer {
  (
    $retry_observer: ident,
    $retry_op: ident,
    $rc: ident,
    $box_observer: ty,
    $box_subscription: ty,
    ($($threads_bound:tt)?),
    $subscribe: ident
  ) => {
    impl<S, O, Item, Err, Config, SD> Observer<Item, Err>
      for $retry_observer<S, O, Err, Config, SD>
    where
      S: Observable<Item, Err, $box_observer> + Clone + 'static $(+ $threads_bound)?,
      S::Unsub: 'static $(+ $threads_bound)?,
      O: Observer<Item, Err> + 'static $(+ $threads_bound)?,
      Err: 'static,
      Config: RetryConfig<Err> + 'static $(+ $threads_bound)?,
      SD: Scheduler<
        OnceTask<
          (
            $retry_op<S, Config, SD>,
            O,
            $rc<Option<$box_subscription>>,
          ),
          NormalReturn<()>,
        >,
      > + 'static $(+ $threads_bound)?,
    {
      fn next(&mut self, value: Item) {
        if self.op.retry_count > 0 && self.op.config.reset_on_success() {
          self.op.retry_count = 0;
        }

        self.observer.next(value);
      }

      fn error(self, err: Err) {
        let Self { mut op, observer, subscription, _hint } = self;

        if op.config.count().is_none_or(|count| op.retry_count < count) {
          op.retry_count += 1;

          let delay = op.config.delay(err, op.retry_count);

          $subscribe(op, observer, subscription, delay);
        } else {
          observer.error(err);
        }
      }

      fn complete(self) {
        self.observer.complete();
      }

      fn is_finished(&self) -> bool {
        self.observer.is_finished()
      }
    }
  };
}

impl_retry_observer!(
  RetryObserver,
  RetryOp,
  MutRc,
  BoxObserver<'static, Item, Err>,
  BoxSubscription<'static>,
  (),
  subscribe
);
impl_retry_observer!(
  RetryThreadsObserver,
  RetryThreadsOp,
  MutArc,
  BoxObserverThreads<Item, Err>,
  BoxSubscriptionThreads,
  (Send),
  subscribe_threads
);

#[cfg(test)]
mod tests {
  use futures::executor::{LocalPool, ThreadPool};

  use crate::{ops::complete_status::CompleteStatus, rc::RcDeref};

  use super::*;

  #[test]
  fn test_retry_with_count() {
    let mut pool = LocalPool::new();
    let scheduler = pool.spawner();

    let count = MutRc::own(0);

    let last_err = MutRc::own(None);
    let values = MutRc::own(vec![]);

    let _ = create(move |mut subscriber: Subscriber<_>| {
      subscriber.next(*count.rc_deref());

      *count.rc_deref_mut() += 1;

      subscriber.error("error");
    })
    .retry_with_config(SimpleRetryConfig::new().count(3), scheduler)
    .on_error({
      let last_err = last_err.clone();

      move |err| {
        *last_err.rc_deref_mut() = Some(err);
      }
    })
    .subscribe({
      let values = values.clone();

      move |v| {
        values.rc_deref_mut().push(v);
      }
    });

    pool.run();

    assert_eq!(*values.rc_deref(), vec![0, 1, 2, 3]);
    assert_eq!(*last_err.rc_deref(), Some("error"));
  }

  #[test]
  fn test_retry_with_count_and_delay() {
    let mut pool = LocalPool::new();
    let scheduler = pool.spawner();

    let count = MutRc::own(0);

    let last_err = MutRc::own(None);
    let values = MutRc::own(vec![]);

    let stamp = Instant::now();

    let elapsed_times = MutRc::own(vec![]);

    let _ = create(move |mut subscriber: Subscriber<_>| {
      subscriber.next(*count.rc_deref());

      *count.rc_deref_mut() += 1;

      subscriber.error("error");
    })
    .retry_with_config(
      SimpleRetryConfig::new()
        .count(3)
        .delay(Duration::from_millis(10)),
      scheduler,
    )
    .on_error({
      let last_err = last_err.clone();

      move |err| {
        *last_err.rc_deref_mut() = Some(err);
      }
    })
    .subscribe({
      let values = values.clone();
      let elapsed_times = elapsed_times.clone();

      move |v| {
        elapsed_times.rc_deref_mut().push(stamp.elapsed());
        values.rc_deref_mut().push(v);
      }
    });

    pool.run();

    assert_eq!(*values.rc_deref(), vec![0, 1, 2, 3]);
    assert_eq!(*last_err.rc_deref(), Some("error"));
    assert!(
      *elapsed_times.rc_deref().first().unwrap() < Duration::from_millis(10)
    );
    assert!(elapsed_times
      .rc_deref()
      .windows(2)
      .all(|w| w[1] - w[0] >= Duration::from_millis(10)));
  }

  #[test]
  fn test_retry_with_reset_on_success() {
    let mut pool = LocalPool::new();
    let scheduler = pool.spawner();

    let count = MutRc::own(0);

    let last_err = MutRc::own(None);
    let values = MutRc::own(vec![]);

    let _ = create(move |mut subscriber: Subscriber<_>| {
      let mut count = count.rc_deref_mut();

      if *count < 3 {
        subscriber.next(*count);
      }

      subscriber.error(format!("error {count}"));

      *count += 1;
    })
    .retry_with_config(
      SimpleRetryConfig::new().count(5).reset_on_success(),
      scheduler,
    )
    .on_error({
      let last_err = last_err.clone();

      move |err| {
        *last_err.rc_deref_mut() = Some(err);
      }
    })
    .subscribe({
      let values = values.clone();

      move |v| {
        values.rc_deref_mut().push(v);
      }
    });

    pool.run();

    assert_eq!(*values.rc_deref(), vec![0, 1, 2]);
    assert_eq!(*last_err.rc_deref(), Some("error 7".to_owned()));
  }

  #[test]
  fn test_retry_threads() {
    let scheduler = ThreadPool::new().unwrap();

    let count = MutArc::own(0);

    let last_err = MutArc::own(None);
    let values = MutArc::own(vec![]);

    let (o, status) = create(move |mut subscriber: SubscriberThreads<_>| {
      subscriber.next(*count.rc_deref());

      *count.rc_deref_mut() += 1;

      subscriber.error("error");
    })
    .retry_with_config_threads(SimpleRetryConfig::new().count(3), scheduler)
    .complete_status();

    o.on_error({
      let last_err = last_err.clone();

      move |err| {
        *last_err.rc_deref_mut() = Some(err);
      }
    })
    .subscribe({
      let values = values.clone();

      move |v| {
        values.rc_deref_mut().push(v);
      }
    });

    CompleteStatus::wait_for_end(status);

    assert_eq!(*values.rc_deref(), vec![0, 1, 2, 3]);
    assert_eq!(*last_err.rc_deref(), Some("error"));
  }
}
