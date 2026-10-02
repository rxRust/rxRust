//! Example: polling a flaky service with exponential backoff
//!
//! A custom `RetryPolicy` decides how long to wait between attempts and when
//! to give up; `timeout` guards against a hung request and `catch_error`
//! turns a final failure into a fallback value.
//!
//! Run with `cargo run --example retry_backoff_poller`.

// The example needs tokio, which is not available on wasm32, so the whole
// program lives behind a target guard and wasm gets an empty `main`.
#[cfg(not(target_arch = "wasm32"))]
mod native {
  use std::{cell::RefCell, rc::Rc, time::Duration};

  use rxrust::{ops::retry::RetryPolicy, prelude::*};

  /// Exponential backoff: `base * factor^attempt`, capped at `max_delay`,
  /// giving up after `max_attempts` retries. Records every delay it chose.
  #[derive(Clone)]
  pub struct ExponentialBackoff {
    pub base: Duration,
    pub factor: u32,
    pub max_delay: Duration,
    pub max_attempts: usize,
    pub chosen: Rc<RefCell<Vec<Duration>>>,
  }

  impl<Err> RetryPolicy<Err> for ExponentialBackoff {
    fn should_retry(&self, _err: &Err, attempt: usize) -> Option<Duration> {
      if attempt >= self.max_attempts {
        return None;
      }
      let delay = (self.base * self.factor.pow(attempt as u32)).min(self.max_delay);
      self.chosen.borrow_mut().push(delay);
      Some(delay)
    }
  }

  /// The error type of the service call.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum FetchError {
    Unavailable,
    TimedOut,
  }

  impl From<TimeoutError> for FetchError {
    fn from(_: TimeoutError) -> Self { FetchError::TimedOut }
  }

  /// A fake service that fails `failures_before_success` times, then answers.
  async fn fetch(
    calls: Rc<RefCell<usize>>, failures_before_success: usize,
  ) -> Result<String, FetchError> {
    let n = {
      let mut calls = calls.borrow_mut();
      *calls += 1;
      *calls
    };
    tokio::time::sleep(Duration::from_millis(1)).await;
    if n <= failures_before_success {
      Err(FetchError::Unavailable)
    } else {
      Ok(format!("payload #{n}"))
    }
  }

  /// Outcome of one polling run.
  #[derive(Debug, PartialEq, Eq)]
  pub struct Outcome {
    pub value: String,
    pub attempts: usize,
    pub delays: Vec<Duration>,
  }

  pub async fn poll(failures_before_success: usize) -> Outcome {
    let calls = Rc::new(RefCell::new(0));
    let policy = ExponentialBackoff {
      base: Duration::from_millis(10),
      factor: 2,
      max_delay: Duration::from_millis(100),
      max_attempts: 4,
      chosen: Rc::new(RefCell::new(Vec::new())),
    };
    let chosen = policy.chosen.clone();
    let calls_for_fetch = calls.clone();

    // Completion is signalled through a oneshot so the caller can await it;
    // the sender sits behind an Rc so the observer stays Clone, which `retry`
    // requires of everything downstream of it.
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
    let done_tx = Rc::new(RefCell::new(Some(done_tx)));
    let value = Rc::new(RefCell::new(None));
    let value_sink = value.clone();

    let _sub = Local::defer(move || {
      Local::from_future_result(fetch(calls_for_fetch.clone(), failures_before_success))
    })
    .timeout(Duration::from_secs(1))
    .retry(policy)
    .catch_error(|e: FetchError| {
      println!("giving up: {e:?}, using fallback");
      Local::of("fallback".to_string())
    })
    .on_complete(move || {
      if let Some(tx) = done_tx.borrow_mut().take() {
        let _ = tx.send(());
      }
    })
    .subscribe(move |v| *value_sink.borrow_mut() = Some(v));

    done_rx.await.expect("pipeline completes");
    let value = value
      .borrow_mut()
      .take()
      .expect("a value was delivered");

    Outcome { value, attempts: *calls.borrow(), delays: chosen.borrow().clone() }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[tokio::test(flavor = "local")]
    async fn recovers_with_backoff() {
      let outcome = poll(2).await;
      assert_eq!(outcome.value, "payload #3");
      assert_eq!(outcome.attempts, 3);
      assert_eq!(outcome.delays, vec![Duration::from_millis(10), Duration::from_millis(20)]);
    }

    #[tokio::test(flavor = "local")]
    async fn falls_back_after_max_attempts() {
      let outcome = poll(usize::MAX).await;
      assert_eq!(outcome.value, "fallback");
      assert_eq!(outcome.attempts, 5);
      assert_eq!(
        outcome.delays,
        vec![
          Duration::from_millis(10),
          Duration::from_millis(20),
          Duration::from_millis(40),
          Duration::from_millis(80)
        ]
      );
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "local")]
async fn main() {
  println!("recovers after two failures: {:#?}", native::poll(2).await);
  println!("gives up on a dead service: {:#?}", native::poll(usize::MAX).await);
}

#[cfg(target_arch = "wasm32")]
fn main() {}
