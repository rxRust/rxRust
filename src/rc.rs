use crate::prelude::*;
use std::ops::DerefMut;
use std::{
  cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut},
  rc::Rc,
  sync::{Arc, Mutex, MutexGuard, TryLockError},
};

pub trait RcDeref {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a>;
}

pub trait TryRcDeref {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a>;
}

pub trait RcDerefMut {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a>;
}

pub trait TryRcDerefMut {
  type Target<'a>
  where
    Self: 'a;
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref_mut<'a>(&'a self) -> Self::Target<'a>;
}

#[derive(Default)]
pub struct MutRc<T>(Rc<RefCell<T>>);
#[derive(Default)]
pub struct MutArc<T>(Arc<Mutex<T>>);

#[derive(Default)]
pub struct BufferedMutRc<T, Item, Err> {
  inner: MutRc<T>,
  buffer: MutRc<Vec<ObserverTrigger<Item, Err>>>,
}
#[derive(Default)]
pub struct BufferedMutArc<T, Item, Err> {
  inner: MutArc<T>,
  buffer: MutArc<Vec<ObserverTrigger<Item, Err>>>,
}

impl<T> MutArc<T> {
  pub fn own(t: T) -> Self { Self(Arc::new(Mutex::new(t))) }
}

impl<T> MutRc<T> {
  pub fn own(t: T) -> Self { Self(Rc::new(RefCell::new(t))) }
}

impl<T, Item, Err> BufferedMutRc<T, Item, Err> {
  pub fn own(t: T) -> Self {
    Self {
      inner: MutRc::own(t),
      buffer: MutRc::own(Vec::new()),
    }
  }
}

impl<T, Item, Err> BufferedMutArc<T, Item, Err> {
  pub fn own(t: T) -> Self {
    Self {
      inner: MutArc::own(t),
      buffer: MutArc::own(Vec::new()),
    }
  }
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

impl<T> TryRcDeref for MutRc<T> {
  type Target<'a>
  where
    Self: 'a,
  = Result<Ref<'a, T>, BorrowError>;
  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a> { self.0.try_borrow() }
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

impl<T> TryRcDeref for MutArc<T> {
  type Target<'a>
  where
    Self: 'a,
  = Result<MutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a> { self.0.try_lock() }
}

impl<T, Item, Err> RcDeref for BufferedMutRc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Ref<'a, T>;
  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a> { self.inner.rc_deref() }
}

impl<T, Item, Err> TryRcDeref for BufferedMutRc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Result<Ref<'a, T>, BorrowError>;
  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a> {
    self.inner.try_rc_deref()
  }
}

impl<T, Item, Err> RcDeref for BufferedMutArc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = MutexGuard<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref<'a>(&'a self) -> Self::Target<'a> { self.inner.rc_deref() }
}

impl<T, Item, Err> TryRcDeref for BufferedMutArc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Result<MutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a> {
    self.inner.try_rc_deref()
  }
}

impl<T> RcDerefMut for MutRc<T> {
  type Target<'a>
  where
    Self: 'a,
  = RefMut<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a> { (*self.0).borrow_mut() }
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

impl<T, Item, Err> RcDerefMut for BufferedMutRc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = RefMut<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a> {
    self.inner.rc_deref_mut()
  }
}

impl<T, Item, Err> RcDerefMut for BufferedMutArc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = MutexGuard<'a, T>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn rc_deref_mut<'a>(&'a self) -> Self::Target<'a> {
    self.inner.rc_deref_mut()
  }
}

impl<T> TryRcDerefMut for MutRc<T> {
  type Target<'a>
  where
    Self: 'a,
  = Result<RefMut<'a, T>, BorrowMutError>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref_mut<'a>(&'a self) -> Self::Target<'a> {
    self.0.try_borrow_mut()
  }
}

impl<T> TryRcDerefMut for MutArc<T> {
  type Target<'a>
  where
    Self: 'a,
  = Result<MutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref_mut<'a>(&'a self) -> Self::Target<'a> { self.0.try_lock() }
}

impl<T, Item, Err> TryRcDerefMut for BufferedMutRc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Result<RefMut<'a, T>, BorrowMutError>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref_mut<'a>(&'a self) -> Self::Target<'a> {
    self.inner.try_rc_deref_mut()
  }
}

impl<T, Item, Err> TryRcDerefMut for BufferedMutArc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Result<MutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref_mut<'a>(&'a self) -> Self::Target<'a> {
    self.inner.try_rc_deref_mut()
  }
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

fn emit_buffer<'a, O, B, Item, Err, T>(mut observer: O, b: &'a B)
where
  O: DerefMut,
  O::Target: Observer<Item = Item, Err = Err>,
  B: RcDerefMut<Target<'a> = T> + 'a,
  T: DerefMut<Target = Vec<ObserverTrigger<Item, Err>>>,
{
  loop {
    let v = b.rc_deref_mut().pop();
    if let Some(to_emit) = v {
      match to_emit {
        ObserverTrigger::Item(v) => observer.next(v),
        ObserverTrigger::Err(err) => observer.error(err),
        ObserverTrigger::Complete => observer.complete(),
      }
    } else {
      break;
    }
  }
}

macro_rules! observer_impl_for_brc {
  ($rc: ident) => {
    impl<T, Item, Err> Observer for $rc<T, Item, Err>
    where
      T: Observer<Item = Item, Err = Err>,
    {
      type Item = Item;
      type Err = Err;
      fn next(&mut self, value: Self::Item) {
        if let Ok(mut inner) = self.try_rc_deref_mut() {
          inner.next(value);
          emit_buffer(inner, &self.buffer);
        } else {
          self
            .buffer
            .rc_deref_mut()
            .push(ObserverTrigger::Item(value));
        }
      }
      fn error(&mut self, err: Self::Err) {
        if let Ok(mut inner) = self.try_rc_deref_mut() {
          inner.error(err);
          emit_buffer(inner, &self.buffer);
        } else {
          self.buffer.rc_deref_mut().push(ObserverTrigger::Err(err));
        }
      }
      fn complete(&mut self) {
        if let Ok(mut inner) = self.try_rc_deref_mut() {
          inner.complete();
          emit_buffer(inner, &self.buffer);
        } else {
          self.buffer.rc_deref_mut().push(ObserverTrigger::Complete);
        }
      }
    }
  };
}

observer_impl_for_brc!(BufferedMutArc);
observer_impl_for_brc!(BufferedMutRc);

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

impl<T: SubscriptionLike, Item, Err> SubscriptionLike
  for BufferedMutRc<T, Item, Err>
{
  #[inline]
  fn unsubscribe(&mut self) { self.rc_deref_mut().unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.rc_deref().is_closed() }
}

impl<T: SubscriptionLike, Item, Err> SubscriptionLike
  for BufferedMutArc<T, Item, Err>
{
  #[inline]
  fn unsubscribe(&mut self) { self.rc_deref_mut().unsubscribe() }

  #[inline]
  fn is_closed(&self) -> bool { self.rc_deref().is_closed() }
}

impl<T> Clone for MutRc<T> {
  #[inline]
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T> Clone for MutArc<T> {
  #[inline]
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T, Item, Err> Clone for BufferedMutRc<T, Item, Err> {
  #[inline]
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      buffer: self.buffer.clone(),
    }
  }
}

impl<T, Item, Err> Clone for BufferedMutArc<T, Item, Err> {
  #[inline]
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      buffer: self.buffer.clone(),
    }
  }
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

impl<T: TearDownSize, Item, Err> TearDownSize for BufferedMutRc<T, Item, Err> {
  #[inline]
  fn teardown_size(&self) -> usize { self.rc_deref().teardown_size() }
}

impl<T: TearDownSize, Item, Err> TearDownSize for BufferedMutArc<T, Item, Err> {
  #[inline]
  fn teardown_size(&self) -> usize { self.rc_deref().teardown_size() }
}
