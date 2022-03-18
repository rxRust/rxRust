use crate::prelude::*;
use crate::{impl_helper::*, impl_local_shared_both};
use std::time::{Duration, Instant};
#[derive(Clone)]
pub struct DebounceOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
  pub(crate) duration: Duration,
}

impl<S: Observable, SD> Observable for DebounceOp<S, SD> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S, SD> DebounceOp<S, SD>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let Self {
      source,
      scheduler,
      duration,
    } = $self;

    source.actual_subscribe($ctx::Rc::own(
      DebounceObserver {
        observer: $observer,
        delay: duration,
        scheduler,
        trailing_value: None,
        last_updated: None,
      },
    ))
  }
  where
    // S::Unsub: 'static,
    @ctx::local_only('o: 'static,)
    S: @ctx::Observable,
    S::Item: Clone @ctx::shared_only(+ Send) + 'static,
    SD: @ctx::Scheduler @ctx::shared_only(+ Send) + 'static
}

struct DebounceObserver<O, S, Item> {
  observer: O,
  scheduler: S,
  delay: Duration,
  trailing_value: Option<Item>,
  last_updated: Option<Instant>,
}

macro_rules! impl_observer {
  ($rc: ident, $sd_bound: ident $(,$send: ident)?) => {
    impl<O, S> Observer for $rc<DebounceObserver<O, S, O::Item>>
    where
      O: Observer $(+ $send)? + 'static,
      O::Item: Clone $(+ $send)?,
      S: $sd_bound $(+ $send)? + 'static,
    {
      type Item = O::Item;
      type Err = O::Err;
      fn next(&mut self, value: Self::Item) {
        let c_observer = self.clone();
        let mut inner = self.rc_deref_mut();
        let updated = Some(Instant::now());
        inner.last_updated = updated;
        inner.trailing_value = Some(value);
        let delay = inner.delay;
        inner.scheduler.schedule(
          move |last| {
            let mut inner = c_observer.rc_deref_mut();
            if let Some(value) = inner.trailing_value.clone() {
              if inner.last_updated == last {
                inner.observer.next(value);
                inner.trailing_value = None;
              }
            }
          },
          Some(delay),
          inner.last_updated,
        );
      }
      fn error(&mut self, err: Self::Err) {
        self.rc_deref_mut().observer.error(err)
      }
      fn complete(&mut self) {
        let mut inner = self.rc_deref_mut();
        if let Some(value) = inner.trailing_value.take() {
          inner.observer.next(value);
        }
        inner.observer.complete();
      }
    }
  };
}

impl_observer!(MutRc, LocalScheduler);
#[cfg(not(feature = "wasm-scheduler"))]
impl_observer!(MutArc, SharedScheduler, Send);
#[cfg(test)]
mod tests {
  use super::*;
  use futures::executor::LocalPool;
  #[test]
  fn smoke_last() {
    let x = MutRc::own(vec![]);
    let x_c = x.clone();
    let mut pool = LocalPool::new();
    let interval =
      observable::interval(Duration::from_millis(2), pool.spawner());
    let spawner = pool.spawner();
    let debounce_subscribe = || {
      let x = x.clone();
      interval
        .clone()
        .take(10)
        .debounce(Duration::from_millis(3), spawner.clone())
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };
    let mut sub = debounce_subscribe();
    pool.run();
    sub.unsubscribe();
    assert_eq!(&*x_c.rc_deref(), &[9]);
  }

  #[test]
  fn smoke_every() {
    let x = MutRc::own(vec![]);
    let x_c = x.clone();
    let mut pool = LocalPool::new();
    let interval =
      observable::interval(Duration::from_millis(3), pool.spawner());
    let spawner = pool.spawner();
    let debounce_subscribe = || {
      let x = x.clone();
      interval
        .clone()
        .take(10)
        .debounce(Duration::from_millis(2), spawner.clone())
        .subscribe(move |v| x.rc_deref_mut().push(v))
    };
    let mut sub = debounce_subscribe();
    pool.run();
    sub.unsubscribe();
    assert_eq!(&*x_c.rc_deref(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
  }

  #[test]
  fn fork_and_shared() {
    use futures::executor::ThreadPool;
    let scheduler = ThreadPool::new().unwrap();
    observable::from_iter(0..10)
      .debounce(Duration::from_nanos(1), scheduler)
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }
}
