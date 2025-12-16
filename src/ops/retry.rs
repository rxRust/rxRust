//! Retry operator implementation
//!
//! This module provides the `retry` operator for resubscribing to an observable
//! when it errors, based on a configurable retry policy.
//!
//! # Examples
//!
//! Simple retry with count:
//!
//! ```rust,no_run
//! use std::{
//!   cell::{Cell, RefCell},
//!   rc::Rc,
//! };
//!
//! use rxrust::prelude::*;
//!
//! let attempts = Rc::new(Cell::new(0));
//!
//! let source = {
//!   let attempts = attempts.clone();
//!   Local::create::<i32, &'static str, _, _>(move |emitter| {
//!     let n = attempts.get() + 1;
//!     attempts.set(n);
//!     if n < 3 {
//!       emitter.error("error");
//!     } else {
//!       emitter.next(1);
//!       emitter.complete();
//!     }
//!     ()
//!   })
//! };
//!
//! let result = Rc::new(RefCell::new(Vec::new()));
//! #[derive(Clone)]
//! struct Record(Rc<RefCell<Vec<i32>>>);
//! impl Observer<i32, &'static str> for Record {
//!   fn next(&mut self, v: i32) { self.0.borrow_mut().push(v); }
//!   fn error(self, _err: &'static str) {}
//!   fn complete(self) {}
//!   fn is_closed(&self) -> bool { false }
//! }
//!
//! source
//!   .retry(3)
//!   .subscribe_with(Record(result.clone()));
//! assert_eq!(&*result.borrow(), &[1]);
//! ```
//!
//! Advanced retry with `RetryConfig`:
//!
//! ```rust,no_run
//! use rxrust::{ops::retry::RetryConfig, prelude::*, scheduler::Duration};
//!
//! Local::throw_err("always fails")
//!   .retry(
//!     RetryConfig::new()
//!       .count(3)
//!       .delay(Duration::from_millis(1))
//!       .reset_on_success(),
//!   )
//!   .subscribe_with({
//!     #[derive(Clone, Copy)]
//!     struct Noop;
//!     impl Observer<(), &'static str> for Noop {
//!       fn next(&mut self, _: ()) {}
//!       fn error(self, _: &'static str) {}
//!       fn complete(self) {}
//!       fn is_closed(&self) -> bool { false }
//!     }
//!     Noop
//!   });
//! ```
//!
//! Custom retry policy based on error type (e.g., HTTP status code):
//!
//! ```rust,no_run
//! use rxrust::{ops::retry::RetryPolicy, prelude::*, scheduler::Duration};
//!
//! #[derive(Clone)]
//! struct HttpRetryPolicy {
//!   max_retries: usize,
//! }
//!
//! impl RetryPolicy<u16> for HttpRetryPolicy {
//!   fn should_retry(&self, err: &u16, attempt: usize) -> Option<Duration> {
//!     if attempt >= self.max_retries {
//!       return None;
//!     }
//!     match *err {
//!       500 | 502 | 503 => Some(Duration::from_millis(1)), // Server errors
//!       429 => Some(Duration::from_millis(1)),             // Rate limit
//!       _ => None,                                         // Client errors, do not retry
//!     }
//!   }
//! }
//!
//! #[derive(Clone, Copy)]
//! struct Noop;
//! impl Observer<(), u16> for Noop {
//!   fn next(&mut self, _: ()) {}
//!   fn error(self, _: u16) {}
//!   fn complete(self) {}
//!   fn is_closed(&self) -> bool { false }
//! }
//!
//! Local::throw_err(500u16)
//!   .retry(HttpRetryPolicy { max_retries: 3 })
//!   .subscribe_with(Noop);
//! ```

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  scheduler::{Duration, Scheduler, Task, TaskState},
  subscription::{IntoBoxedSubscription, Subscription},
};

/// Policy for determining whether to retry an error.
///
/// This trait allows customizing the retry behavior. Simple policies like
/// `usize` (count) and complex policies like `RetryConfig` (count + delay +
/// reset) are supported. Users can also implement this trait for custom retry
/// logic.
///
/// # Custom Policy Example
///
/// ```rust
/// use rxrust::{ops::retry::RetryPolicy, prelude::*, scheduler::Duration};
///
/// #[derive(Clone)]
/// struct WebErrorRetry;
///
/// impl RetryPolicy<u16> for WebErrorRetry {
///   fn should_retry(&self, status: &u16, attempt: usize) -> Option<Duration> {
///     if attempt >= 3 {
///       return None;
///     }
///     match status {
///       500..=599 => Some(Duration::from_millis(500)), // Server error: retry
///       429 => Some(Duration::from_secs(2)),           // Too many requests: retry with backoff
///       _ => None,                                     // Client error: do not retry
///     }
///   }
/// }
/// ```
pub trait RetryPolicy<Err>: Clone {
  /// Determines if a retry should occur and the delay before retrying.
  ///
  /// # Arguments
  ///
  /// * `err` - The error that occurred.
  /// * `attempt` - The number of retries already attempted (0-indexed).
  ///   - 0 means this is the first error (first retry check).
  ///   - 1 means this is the second error, etc.
  ///
  /// # Returns
  ///
  /// * `Some(duration)` - Retry after the specified duration.
  /// * `None` - Do not retry, propagate the error.
  fn should_retry(&self, err: &Err, attempt: usize) -> Option<Duration>;

  /// Whether to reset the retry count when a value is successfully emitted.
  ///
  /// If this returns `true`, the retry counter (attempt index) will be reset to
  /// 0 whenever the source observable emits a `next` value. This is useful
  /// for "connection" scenarios where a successful value means the connection
  /// is healthy.
  fn reset_on_success(&self) -> bool { false }
}

impl<Err> RetryPolicy<Err> for usize {
  fn should_retry(&self, _err: &Err, attempt: usize) -> Option<Duration> {
    if attempt < *self { Some(Duration::ZERO) } else { None }
  }
}

/// A configuration struct for Retry.
///
/// Builder pattern for creating a retry policy.
///
/// Allows configuring:
/// - Maximum retry count
/// - Delay between retries
/// - Whether to reset the retry count on successful emission
///
/// # Examples
///
/// ```rust
/// use rxrust::{ops::retry::RetryConfig, scheduler::Duration};
///
/// let config = RetryConfig::new()
///   .count(5)
///   .delay(Duration::from_secs(1))
///   .reset_on_success();
/// ```
#[derive(Clone)]
pub struct RetryConfig {
  count: Option<usize>,
  delay: Option<Duration>,
  reset_on_success: bool,
}

impl RetryConfig {
  /// Creates a new default configuration (no retries, no delay).
  pub fn new() -> Self { Self { count: None, delay: None, reset_on_success: false } }

  /// Sets the maximum number of retry attempts.
  ///
  /// This specifies how many times to retry upon failure.
  /// For example, `count(3)` allows for 3 retries, resulting in a maximum
  /// of 4 total subscription attempts (1 initial + 3 retries).
  pub fn count(mut self, count: usize) -> Self {
    self.count = Some(count);
    self
  }

  /// Sets the delay duration between retries.
  ///
  /// If specified, the operator will wait for this duration before
  /// resubscribing to the source.
  pub fn delay(mut self, delay: Duration) -> Self {
    self.delay = Some(delay);
    self
  }

  /// Enables resetting the retry count when a value is successfully emitted.
  ///
  /// When enabled, if the source emits a value, the retry counter is reset.
  /// This allows for indefinite retries as long as the connection stays healthy
  /// for at least one emission between failures.
  pub fn reset_on_success(mut self) -> Self {
    self.reset_on_success = true;
    self
  }
}

impl Default for RetryConfig {
  fn default() -> Self { Self::new() }
}

impl<Err> RetryPolicy<Err> for RetryConfig {
  fn should_retry(&self, _err: &Err, attempt: usize) -> Option<Duration> {
    if let Some(count) = self.count
      && attempt >= count
    {
      return None;
    }
    Some(self.delay.unwrap_or(Duration::ZERO))
  }

  fn reset_on_success(&self) -> bool { self.reset_on_success }
}

/// The Retry operator struct.
#[derive(Clone)]
pub struct Retry<S, P, Sch> {
  pub source: S,
  pub policy: P,
  pub scheduler: Sch,
}

/// Observer for retry operator.
///
/// # Internal Implementation Detail
///
/// This struct is public to satisfy type trait bounds in the public API,
/// but it is considered an internal implementation detail of the `retry`
/// operator. Users should not instantiate or interact with this struct
/// directly.
pub struct RetryObserver<S, P, ObserverCtx>
where
  ObserverCtx: Context,
{
  source: S,
  policy: P,
  observer: ObserverCtx,
  attempts: usize,
  serial_subscription: ObserverCtx::RcMut<Option<ObserverCtx::BoxedSubscription>>,
  // use function pointer to avoid circular type bounds requirements
  subscribe_fn: fn(Self),
}

impl<S, P, Sch> ObservableType for Retry<S, P, Sch>
where
  S: ObservableType,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, P, O> ObservableType for RetryObserver<S, P, O>
where
  S: ObservableType,
  O: Context,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, P, Sch, Ctx> CoreObservable<Ctx> for Retry<S, P, Sch>
where
  Ctx: Context,
  S: CoreObservable<Ctx::With<RetryObserver<S, P, Ctx>>> + Clone,
  S::Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
  Ctx::RcMut<Option<Ctx::BoxedSubscription>>: Subscription,
{
  type Unsub = Ctx::RcMut<Option<Ctx::BoxedSubscription>>;
  fn subscribe(self, observer: Ctx) -> Self::Unsub {
    let serial_subscription = Ctx::RcMut::from(None);
    let retry_observer = RetryObserver {
      source: self.source,
      policy: self.policy,
      observer,
      attempts: 0,
      serial_subscription: serial_subscription.clone(),
      subscribe_fn: RetryObserver::subscribe_impl,
    };
    let subscription = retry_observer
      .source
      .clone()
      .subscribe(Ctx::lift(retry_observer));
    *serial_subscription.rc_deref_mut() = Some(subscription.into_boxed());
    serial_subscription
  }
}

impl<S, P, Ctx, Item, Err> Observer<Item, Err> for RetryObserver<S, P, Ctx>
where
  Self: Clone,
  P: RetryPolicy<Err>,
  Ctx: Context<Scheduler: Scheduler<Task<Option<Self>>>> + Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    if self.attempts > 0 && self.policy.reset_on_success() {
      self.attempts = 0;
    }
    self.observer.next(value);
  }

  fn error(mut self, err: Err) {
    if let Some(delay) = self.policy.should_retry(&err, self.attempts) {
      self.attempts += 1;
      let sch = self.observer.scheduler().clone();
      sch.schedule(
        Task::new(Some(self.clone()), |this| {
          if let Some(observer) = this.take() {
            (observer.subscribe_fn)(observer);
          }
          TaskState::Finished
        }),
        Some(delay),
      );
    } else {
      self.observer.error(err);
    }
  }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, P, Ctx> RetryObserver<S, P, Ctx>
where
  Ctx: Context,
  S: CoreObservable<Ctx::With<Self>> + Clone,
  S::Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>,
{
  fn subscribe_impl(self) {
    let serial_subscription = self.serial_subscription.clone();
    if let Some(sub) = serial_subscription.rc_deref_mut().take() {
      sub.unsubscribe();
    }
    let source = self.source.clone();
    let subscription = source.subscribe(Ctx::lift(self));
    *serial_subscription.rc_deref_mut() = Some(subscription.into_boxed());
  }
}

impl<S, P, Ctx> Clone for RetryObserver<S, P, Ctx>
where
  S: Clone,
  P: Clone,
  Ctx: Clone + Context,
{
  fn clone(&self) -> Self {
    Self {
      source: self.source.clone(),
      policy: self.policy.clone(),
      observer: self.observer.clone(),
      attempts: self.attempts,
      serial_subscription: self.serial_subscription.clone(),
      subscribe_fn: self.subscribe_fn,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::*;
  use crate::{observer::Emitter, prelude::*};

  #[rxrust_macro::test]
  fn test_retry_simple() {
    let received = Rc::new(RefCell::new(Vec::new()));
    let received_c = received.clone();

    // Simple test: emit values then complete
    Local::from_iter([1, 2, 3])
      .retry(3)
      .subscribe(move |v| received_c.borrow_mut().push(v));

    assert_eq!(*received.borrow(), vec![1, 2, 3]);
  }

  #[rxrust_macro::test]
  fn test_retry_config() {
    let config = RetryConfig::new()
      .count(3)
      .delay(Duration::from_millis(100));
    assert_eq!(config.should_retry(&"error", 0), Some(Duration::from_millis(100)));
    assert_eq!(config.should_retry(&"error", 2), Some(Duration::from_millis(100)));
    assert_eq!(config.should_retry(&"error", 3), None);
  }

  #[rxrust_macro::test]
  fn test_retry_policy_usize() {
    let policy: usize = 3;
    assert_eq!(policy.should_retry(&"error", 0), Some(Duration::ZERO));
    assert_eq!(policy.should_retry(&"error", 2), Some(Duration::ZERO));
    assert_eq!(policy.should_retry(&"error", 3), None);
  }

  #[rxrust_macro::test]
  fn test_retry_error_propagation() {
    TestScheduler::init();
    let error_seen = Rc::new(RefCell::new(None));
    let error_c = error_seen.clone();

    TestCtx::throw_err("test error")
      .retry(2)
      .on_error(move |e| *error_c.borrow_mut() = Some(e))
      .subscribe(|_| {});

    // Retry delay is 0, so should be scheduled immediately (or very close).
    // TestScheduler::flush() executes all tasks scheduled at current time or
    // before.
    TestScheduler::flush();

    assert_eq!(*error_seen.borrow(), Some("test error"));
  }

  #[rxrust_macro::test]
  fn test_retry_with_count_and_delay() {
    TestScheduler::init();
    let count = Rc::new(RefCell::new(0));
    let last_err = Rc::new(RefCell::new(None));
    let values = Rc::new(RefCell::new(vec![]));

    let count_clone = count.clone();
    TestCtx::create(move |emitter: &mut dyn Emitter<i32, &str>| {
      let c = *count_clone.borrow();
      emitter.next(c);
      *count_clone.borrow_mut() += 1;
      emitter.error("error");
    })
    .retry(
      RetryConfig::new()
        .count(3)
        .delay(Duration::from_millis(10)),
    )
    .on_error({
      let last_err = last_err.clone();
      move |err| *last_err.borrow_mut() = Some(err)
    })
    .subscribe({
      let values = values.clone();
      move |v| {
        values.borrow_mut().push(v);
      }
    });

    // Initial subscription
    assert_eq!(*values.borrow(), vec![0]);

    TestScheduler::advance_by(Duration::from_millis(9));
    assert_eq!(*values.borrow(), vec![0]);

    TestScheduler::advance_by(Duration::from_millis(1));
    assert_eq!(*values.borrow(), vec![0, 1]);

    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(*values.borrow(), vec![0, 1, 2]);

    TestScheduler::advance_by(Duration::from_millis(10));
    assert_eq!(*values.borrow(), vec![0, 1, 2, 3]);
    assert_eq!(*last_err.borrow(), Some("error"));
  }

  #[rxrust_macro::test]
  fn test_retry_with_reset_on_success() {
    TestScheduler::init();
    let count = Rc::new(RefCell::new(0));
    let last_err = Rc::new(RefCell::new(None));
    let values = Rc::new(RefCell::new(vec![]));

    let count_clone = count.clone();
    TestCtx::create(move |emitter: &mut dyn Emitter<i32, String>| {
      let mut c = count_clone.borrow_mut();
      if *c < 3 {
        emitter.next(*c);
      }
      emitter.error(format!("error {}", *c));
      *c += 1;
    })
    .retry(RetryConfig::new().count(5).reset_on_success())
    .on_error({
      let last_err = last_err.clone();
      move |err| *last_err.borrow_mut() = Some(err)
    })
    .subscribe({
      let values = values.clone();
      move |v| values.borrow_mut().push(v)
    });

    TestScheduler::flush();

    assert_eq!(*values.borrow(), vec![0, 1, 2]);
    assert_eq!(*last_err.borrow(), Some("error 7".to_owned()));
  }
}
