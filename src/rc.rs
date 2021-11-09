use crate::prelude::*;
use std::{
  cell::{Ref, RefCell, RefMut},
  rc::Rc,
  sync::{Arc, Mutex, MutexGuard},
};

pub trait RcDeref {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a>;
}

pub trait RcDerefMut {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a>;
}

#[derive(Default)]
pub struct MutRc<T>(Rc<RefCell<T>>);
#[derive(Default)]
pub struct MutArc<T>(Arc<Mutex<T>>);

impl<T> MutArc<T> {
  pub fn own(t: T) -> Self { Self(Arc::new(Mutex::new(t))) }
}

impl<T> MutRc<T> {
  pub fn own(t: T) -> Self { Self(Rc::new(RefCell::new(t))) }
}

impl<T> RcDeref for MutRc<T> {
  type Target<'a>
  where
    Self: 'a,
  = Ref<'a, T>;
  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a> { self.0.borrow() }
}

impl<T> RcDeref for MutArc<T> {
  type Target<'a>
  where
    Self: 'a,
  = MutexGuard<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a> { self.0.lock().unwrap() }
}

impl<T> RcDerefMut for MutRc<T> {
  type Target<'a>
  where
    Self: 'a,
  = RefMut<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a> { self.0.borrow_mut() }
}

impl<T> RcDerefMut for MutArc<T> {
  type Target<'a>
  where
    Self: 'a,
  = MutexGuard<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a> { self.0.lock().unwrap() }
}

macro_rules! observer_impl {
  ($rc: ident) => {
    impl<T> Observer for $rc<T>
    where
      T: Observer,
    {
      type Item = T::Item;
      type Err = T::Err;
      fn next(&mut self, value: Self::Item) { self.rc_deref_mut().next(value) }
      fn error(&mut self, err: Self::Err) { self.rc_deref_mut().error(err); }
      fn complete(&mut self) { self.rc_deref_mut().complete(); }
    }
  };
}

observer_impl!(MutArc);
observer_impl!(MutRc);

macro_rules! rc_subscription_impl {
  ($rc: ident) => {
    impl<T: SubscriptionLike> SubscriptionLike for $rc<T> {
      #[inline]
      fn unsubscribe(&mut self) { self.rc_deref_mut().unsubscribe() }

      #[inline]
      fn is_closed(&self) -> bool { self.rc_deref().is_closed() }
    }
  };
}

rc_subscription_impl!(MutArc);
rc_subscription_impl!(MutRc);

impl<T> Clone for MutRc<T> {
  #[inline]
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T> Clone for MutArc<T> {
  #[inline]
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

macro_rules! impl_teardown_size {
  ($t: ident) => {
    impl<T: TearDownSize> TearDownSize for $t<T> {
      #[inline]
      fn teardown_size(&self) -> usize { self.rc_deref().teardown_size() }
    }
  };
}
impl_teardown_size!(MutRc);
impl_teardown_size!(MutArc);
