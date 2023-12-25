use std::{
  cell::{Ref, RefCell, RefMut},
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::{Arc, Mutex, MutexGuard},
};

pub trait RcDeref: Clone {
  type Target;
  type Ref<'a>: Deref<Target = Self::Target>
  where
    Self: 'a;

  fn rc_deref(&self) -> Self::Ref<'_>;
}

pub trait RcDerefMut: Clone {
  type Target;
  type MutRef<'a>: DerefMut<Target = Self::Target>
  where
    Self: 'a;
  fn rc_deref_mut(&self) -> Self::MutRef<'_>;
}

pub trait AssociatedRefPtr {
  type Rc<T>: RcDeref<Target = T> + RcDerefMut<Target = T> + From<T>;
}

#[derive(Default)]
pub struct MutRc<T>(Rc<RefCell<T>>);
#[derive(Default)]
pub struct MutArc<T>(Arc<Mutex<T>>);

impl<T> From<T> for MutArc<T> {
  fn from(t: T) -> Self {
    Self(Arc::from(Mutex::from(t)))
  }
}

impl<T> From<T> for MutRc<T> {
  fn from(t: T) -> Self {
    Self(Rc::from(RefCell::from(t)))
  }
}

impl<T> MutArc<T> {
  pub fn own(t: T) -> Self {
    Self::from(t)
  }
}

impl<T> MutRc<T> {
  pub fn own(t: T) -> Self {
    Self::from(t)
  }
}

impl<T> RcDeref for MutRc<T> {
  type Target = T;
  type Ref<'a >  = Ref<'a, T> where Self: 'a;

  #[inline]
  fn rc_deref(&self) -> Self::Ref<'_> {
    self.0.borrow()
  }
}

impl<T> RcDeref for MutArc<T> {
  type Target = T;
  type Ref<'a> = MutexGuard<'a, T> where Self:'a;

  #[inline]
  fn rc_deref(&self) -> Self::Ref<'_> {
    self.0.lock().unwrap()
  }
}

impl<T> RcDerefMut for MutRc<T> {
  type Target = T;
  type MutRef<'a> = RefMut<'a, T> where Self:'a;

  #[inline]
  fn rc_deref_mut(&self) -> Self::MutRef<'_> {
    (*self.0).borrow_mut()
  }
}

impl<T> RcDerefMut for MutArc<T> {
  type Target = T;
  type MutRef<'a> = MutexGuard<'a, T> where Self:'a;

  #[inline]
  fn rc_deref_mut(&self) -> Self::MutRef<'_> {
    self.0.lock().unwrap()
  }
}

impl<T> Clone for MutRc<T> {
  #[inline]
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> Clone for MutArc<T> {
  #[inline]
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}
