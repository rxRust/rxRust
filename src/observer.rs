use crate::inner_deref::InnerDerefMut;

/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer<Item, Err> {
  fn next(&mut self, value: Item);
  fn error(&mut self, err: Err);
  fn complete(&mut self);
  fn is_stopped(&self) -> bool;
}

#[doc(hidden)]
pub(crate) macro next_proxy_impl(
  $item: ident, $($name:tt $($parentheses:tt)?) .+)
{
  #[inline]
  fn next(&mut self, value: $item) {
    self.$($name$($parentheses)?).+.next(value);
  }
}

#[doc(hidden)]
pub(crate) macro error_proxy_impl(
  $err: ident, $($name:tt $($parentheses:tt)?) .+)
{
  #[inline]
  fn error(&mut self, err: $err) {
    self.$($name$($parentheses)?).+.error(err);
  }
}

#[doc(hidden)]
pub(crate) macro complete_proxy_impl($($name:tt $($parentheses:tt)?) .+) {
  #[inline]
  fn complete(&mut self) { self.$($name$($parentheses)?).+.complete(); }
}

#[doc(hidden)]
pub(crate) macro is_stopped_proxy_impl($($name:tt $($parentheses:tt)?) .+) {
  #[inline]
  fn is_stopped(&self) -> bool { self.$($name$($parentheses)?).+.is_stopped() }
}

impl<Item, Err, T> Observer<Item, Err> for T
where
  T: InnerDerefMut,
  T::Target: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.inner_deref_mut().next(value) }
  fn error(&mut self, err: Err) { self.inner_deref_mut().error(err); }
  fn complete(&mut self) { self.inner_deref_mut().complete(); }
  fn is_stopped(&self) -> bool { self.inner_deref().is_stopped() }
}
