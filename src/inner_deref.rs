use std::{
  cell::{Ref, RefCell, RefMut},
  ops::{Deref, DerefMut},
  rc::Rc,
  sync::{Arc, Mutex, MutexGuard},
};

pub trait InnerDeref {
  type Target;
  #[rustfmt::skip]
  type Deref<'r>: Deref<Target = Self::Target> where Self::Target: 'r;
  fn inner_deref(&self) -> Self::Deref<'_>;
}

pub trait InnerDerefMut: InnerDeref {
  #[rustfmt::skip]
  type DerefMut<'r>: DerefMut<Target = Self::Target> where Self::Target: 'r;
  fn inner_deref_mut(&mut self) -> Self::DerefMut<'_>;
}

impl<T> InnerDeref for Rc<RefCell<T>> {
  type Target = T;
  #[rustfmt::skip]
  type Deref<'r> where T:'r = Ref<'r, T>;

  #[inline]
  fn inner_deref(&self) -> Self::Deref<'_> { self.borrow() }
}

impl<T> InnerDerefMut for Rc<RefCell<T>> {
  #[rustfmt::skip]
  type DerefMut<'r> where T: 'r = RefMut<'r, T>;
  #[inline]
  fn inner_deref_mut(&mut self) -> Self::DerefMut<'_> { self.borrow_mut() }
}

impl<T> InnerDeref for Arc<Mutex<T>> {
  type Target = T;
  #[rustfmt::skip]
  type Deref<'r> where T:'r = MutexGuard<'r, T>;
  #[inline]
  fn inner_deref(&self) -> Self::Deref<'_> { self.lock().unwrap() }
}

impl<T> InnerDerefMut for Arc<Mutex<T>> {
  #[rustfmt::skip]
  type DerefMut<'r> where T: 'r = MutexGuard<'r, T>;
  #[inline]
  fn inner_deref_mut(&mut self) -> Self::DerefMut<'_> { self.lock().unwrap() }
}

macro ptr_impl_inner_deref($target: ident, $ty: ty) {
  impl<$target> InnerDeref for $ty {
    #[rustfmt::skip]
    type Deref<'r> where $target:'r =&'r $target;
    type Target = $target;
    fn inner_deref(&self) -> Self::Deref<'_> { self }
  }
}

macro ptr_impl_inner_deref_mut($target: ident, $ty: ty) {
  impl<$target> InnerDerefMut for $ty {
    #[rustfmt::skip]
    type DerefMut<'r> where $target: 'r = &'r mut $target;
    #[inline]
    fn inner_deref_mut(&mut self) -> Self::DerefMut<'_> { self }
  }
}

ptr_impl_inner_deref!(T, Box<T>);
ptr_impl_inner_deref_mut!(T, Box<T>);
