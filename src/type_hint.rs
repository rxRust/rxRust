use std::marker::PhantomData;

pub struct TypeHint<T>(PhantomData<*const T>);

impl<T> TypeHint<T> {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }
}

impl<T> Default for TypeHint<T> {
  fn default() -> Self {
    TypeHint(PhantomData)
  }
}

unsafe impl<T> Sync for TypeHint<T> {}
unsafe impl<T> Send for TypeHint<T> {}

impl<T> Clone for TypeHint<T> {
  #[inline]
  fn clone(&self) -> Self {
    Self::new()
  }
}
