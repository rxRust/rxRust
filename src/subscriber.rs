//! subscriber is a object implemented both `Observer` and `Subscription`.
//!
use crate::{
  observer::Observer,
  prelude::Subscription,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};

pub struct Subscriber<O>(MutRc<Option<O>>);
pub struct SubscriberThreads<O>(MutArc<Option<O>>);

impl<O> Subscriber<O> {
  #[inline]
  pub fn new<Item, Err>(observer: Option<O>) -> Self
  where
    O: Observer<Item, Err>,
  {
    Self(MutRc::own(observer))
  }
}

impl<O> SubscriberThreads<O> {
  #[inline]
  pub fn new<Item, Err>(observer: Option<O>) -> Self
  where
    O: Observer<Item, Err> + Send,
  {
    Self(MutArc::own(observer))
  }
}

pub trait Publisher<Item, Err> {
  fn p_next(&mut self, value: Item);
  fn p_error(self: Box<Self>, err: Err);
  fn p_complete(self: Box<Self>);
  fn p_unsubscribe(self: Box<Self>);
  fn p_is_closed(&self) -> bool;
}

macro_rules! impl_subscriber {
  ($subscriber: ident) => {
    impl<Item, Err, O> Observer<Item, Err> for $subscriber<O>
    where
      O: Observer<Item, Err>,
    {
      #[inline]
      fn next(&mut self, value: Item) {
        self.0.next(value);
      }

      #[inline]
      fn error(self, err: Err) {
        self.0.error(err)
      }

      #[inline]
      fn complete(self) {
        self.0.complete()
      }

      #[inline]
      fn is_finished(&self) -> bool {
        self.0.is_finished()
      }
    }

    impl<O> Subscription for $subscriber<O> {
      #[inline]
      fn unsubscribe(self) {
        self.0.rc_deref_mut().take();
      }

      #[inline]
      fn is_closed(&self) -> bool {
        self.0.rc_deref().is_none()
      }
    }

    impl<Item, Err, O> Publisher<Item, Err> for $subscriber<O>
    where
      O: Observer<Item, Err>,
    {
      #[inline]
      fn p_next(&mut self, value: Item) {
        self.next(value);
      }
      #[inline]
      fn p_error(self: Box<Self>, err: Err) {
        self.error(err);
      }
      #[inline]
      fn p_complete(self: Box<Self>) {
        self.complete();
      }
      #[inline]
      fn p_unsubscribe(self: Box<Self>) {
        self.unsubscribe()
      }
      fn p_is_closed(&self) -> bool {
        self.is_finished() || self.is_closed()
      }
    }

    impl<O> Clone for $subscriber<O> {
      #[inline]
      fn clone(&self) -> Self {
        Self(self.0.clone())
      }
    }
  };
}

impl_subscriber!(Subscriber);
impl_subscriber!(SubscriberThreads);
