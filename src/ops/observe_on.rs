use crate::prelude::*;
use crate::scheduler::Scheduler;
use std::sync::Arc;

pub trait ObserveOn {
  fn observe_on<SD>(self, scheduler: SD) -> ObserveOnOp<Self, SD>
  where
    Self: Sized,
  {
    ObserveOnOp {
      source: self,
      scheduler,
    }
  }
}

pub struct ObserveOnOp<S, SD> {
  source: S,
  scheduler: SD,
}

impl<S> ObserveOn for S where S: ImplSubscribable {}

impl<S, SD> ImplSubscribable for ObserveOnOp<S, SD>
where
  S: ImplSubscribable,
  S::Item: Clone + Send + Sync + 'static,
  S::Err: Clone + Send + Sync + 'static,
  SD: Scheduler + Send + Sync + 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  fn subscribe_return_state(
    self,
    subscribe: impl RxFn(
        RxValue<&'_ Self::Item, &'_ Self::Err>,
      ) -> RxReturn<Self::Err>
      + Send
      + Sync
      + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let scheduler = self.scheduler;
    let subscribe = Arc::new(subscribe);

    self.source.subscribe_return_state(RxFnWrapper::new(
      move |v: RxValue<&'_ Self::Item, &'_ Self::Err>| {
        scheduler.schedule(
          |value| {
            if let Some((rv, subscribe)) = value {
              subscribe.call((rv.as_ref(),));
            }
          },
          Some((v.to_owned(), subscribe.clone())),
        );
        RxReturn::Continue
      },
    ))
  }
}
