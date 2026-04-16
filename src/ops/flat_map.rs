use std::marker::PhantomData;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  ops::{map::Map, merge_all::MergeAll},
};

#[derive(Clone)]
pub struct FlatMap<S, F, Inner> {
  pub source: S,
  pub func: F,
  pub concurrent: usize,
  _inner: PhantomData<fn() -> Inner>,
}

impl<S, F, Inner> FlatMap<S, F, Inner> {
  pub fn new(source: S, func: F, concurrent: usize) -> Self {
    Self { source, func, concurrent, _inner: PhantomData }
  }
}

impl<S, F, Inner> ObservableType for FlatMap<S, F, Inner>
where
  S: ObservableType,
  F: for<'a> FnMut(S::Item<'a>) -> Inner,
  Inner: Context<Inner: ObservableType<Err = S::Err>>,
{
  type Item<'a>
    = <Inner::Inner as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = S::Err;
}

impl<S, F, Inner, C> CoreObservable<C> for FlatMap<S, F, Inner>
where
  C: Context,
  S: ObservableType,
  F: for<'a> FnMut(S::Item<'a>) -> Inner,
  Inner: Context<Inner: ObservableType<Err = S::Err>>,
  MergeAll<Map<S, F>>: CoreObservable<C>,
{
  type Unsub = <MergeAll<Map<S, F>> as CoreObservable<C>>::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    MergeAll { source: Map { source: self.source, func: self.func }, concurrent: self.concurrent }
      .subscribe(context)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc, time::Duration};

  use crate::{prelude::*, scheduler::LocalScheduler};

  #[rxrust_macro::test(local)]
  async fn flat_map_accepts_from_future_inner() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::of(Local::from_future(async { 42_u8 }))
      .flat_map(|v| v)
      .subscribe(move |v| *result_clone.borrow_mut() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.borrow(), Some(42));
  }

  #[rxrust_macro::test(local)]
  async fn concat_map_accepts_from_future_inner() {
    let result = Rc::new(RefCell::new(None));
    let result_clone = result.clone();

    Local::of(Local::from_future(async { 7_u8 }))
      .concat_map(|v| v)
      .subscribe(move |v| *result_clone.borrow_mut() = Some(v));

    LocalScheduler
      .sleep(Duration::from_millis(0))
      .await;

    assert_eq!(*result.borrow(), Some(7));
  }
}
