use crate::prelude::*;
use std::borrow::BorrowMut;
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
  inner: Rc<RefCell<T>>,
  buffer: Rc<RefCell<Vec<ObserverTrigger<Item, Err>>>>,
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
      inner: Rc::new(RefCell::new(t)),
      buffer: Rc::new(RefCell::new(Vec::new())),
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
  fn rc_deref<'a>(&'a self) -> Self::Target<'a> { self.inner.borrow() }
}

impl<T, Item, Err> TryRcDeref for BufferedMutRc<T, Item, Err> {
  type Target<'a>
  where
    Self: 'a,
  = Result<Ref<'a, T>, BorrowError>;
  #[inline]
  #[allow(clippy::needless_lifetimes)]
  fn try_rc_deref<'a>(&'a self) -> Self::Target<'a> { self.inner.try_borrow() }
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
    (*self.inner).borrow_mut()
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
    self.inner.try_borrow_mut()
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

// impl<T, Item, Err> BufferedMutRc<T, Item, Err>
// where
//   T: Observer<Item = Item, Err = Err>,
// {
//   pub fn emit_buffer(&self) {
//     let mut observer = self.rc_deref_mut();
//     loop {
//       let v = (*self.buffer).borrow_mut().pop();
//       if let Some(to_emit) = v {
//         match to_emit {
//           ObserverTrigger::Item(v) => observer.next(v),
//           ObserverTrigger::Err(err) => observer.error(err),
//           ObserverTrigger::Complete => observer.complete(),
//         }
//       } else {
//         break;
//       }
//     }
//   }
// }

fn emit_buffer<'a, O, Item, Err>(
  mut observer: O,
  b: &'a mut Vec<ObserverTrigger<Item, Err>>,
) where
  O: DerefMut,
  O::Target: Observer<Item = Item, Err = Err>,
{
  loop {
    let v = b.pop();
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

impl<T, Item, Err> Observer for BufferedMutRc<T, Item, Err>
where
  T: Observer<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Self::Item) {
    if let Ok(mut inner) = self.try_rc_deref_mut() {
      inner.next(value);
      emit_buffer(inner, &mut (*self.buffer).borrow_mut());
    } else {
      (*self.buffer)
        .borrow_mut()
        .push(ObserverTrigger::Item(value));
    }
  }

  fn error(&mut self, err: Self::Err) {
    if let Ok(mut inner) = self.try_rc_deref_mut() {
      inner.error(err);
      emit_buffer(inner, &mut (*self.buffer).borrow_mut());
    } else {
      (*self.buffer).borrow_mut().push(ObserverTrigger::Err(err));
    }
  }

  fn complete(&mut self) {
    if let Ok(mut inner) = self.try_rc_deref_mut() {
      inner.complete();
      emit_buffer(inner, &mut (*self.buffer).borrow_mut());
    } else {
      (*self.buffer).borrow_mut().push(ObserverTrigger::Complete);
    }
  }
}

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
      inner: Rc::clone(&self.inner),
      buffer: Rc::clone(&self.buffer),
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
  fn teardown_size(&self) -> usize { self.inner.borrow().teardown_size() }
}
