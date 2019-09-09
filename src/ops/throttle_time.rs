use crate::ops::Delay;
use crate::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(PartialEq, Clone, Copy)]
pub enum ThrottleEdge {
  Tailing,
  Leading,
}

pub struct ThrottleTimeOp<S> {
  source: S,
  duration: Duration,
  edge: ThrottleEdge,
}

pub trait ThrottleTime
where
  Self: Sized,
{
  fn throttle_time(
    self,
    duration: Duration,
    edge: ThrottleEdge,
  ) -> ThrottleTimeOp<Self> {
    ThrottleTimeOp {
      source: self,
      duration,
      edge,
    }
  }
}

impl<S> RawSubscribable for ThrottleTimeOp<S>
where
  S: RawSubscribable + Send + Sync,
  S::Item: Send + Clone + 'static,
  S::Err: Sync,
{
  type Item = S::Item;
  type Err = S::Err;

  fn raw_subscribe(
    self,
    subscribe: impl RxFn(RxValue<&'_ Self::Item, &'_ Self::Err>)
    + Send
    + Sync
    + 'static,
  ) -> Box<dyn Subscription + Send + Sync> {
    let Self {
      source,
      duration,
      edge,
    } = self;
    let proxy = SubscriptionProxy::new();
    let c_proxy = proxy.clone();
    let trailing_value = Arc::new(Mutex::new(None));
    let throttled = Arc::new(Mutex::new(None));
    let throttle_subscribe =
      move |v: RxValue<&'_ Self::Item, &'_ Self::Err>| {
        let mut td = throttled.lock().unwrap();
        if edge == ThrottleEdge::Tailing {
          match v {
            RxValue::Next(value) => {
              *trailing_value.lock().unwrap() = Some(value.clone())
            }
            _ => subscribe.call((v,)),
          }
        }

        if td.is_none() {
          if edge == ThrottleEdge::Leading {
            subscribe.call((v,));
          }
          let ctd = throttled.clone();
          let c_tailing_value = trailing_value.clone();
          let sub =
            observable::empty()
              .delay(duration)
              .subscribe(move |v: &()| {
                let mut tv = c_tailing_value.lock().unwrap();
                if edge == ThrottleEdge::Tailing {
                  if let Some(value) = tv.take() {
                    // subscribe.call((RxValue::Next(&value),));
                  }
                }
                ctd.lock().unwrap().take();
              });
          let sub = Arc::new(sub);
          *td = Some(sub.clone());
          // proxy.proxy(Box::new(sub));
        }
      };

    source.raw_subscribe(RxFnWrapper::new(throttle_subscribe));
    Box::new(c_proxy)
  }
}
