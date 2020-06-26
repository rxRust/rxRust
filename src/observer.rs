use crate::subscription::Publisher;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

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
}

#[doc(hidden)]
/// auto impl a proxy observer
/// observer_proxy_impl!(
///   type          // give the type you want to implement for
///   , {path}      // the path to access to the actual observer,
///   , item        // the generic Item name
///   , err         // the generic Err name
///   , host_type?  // options, give the host type of the actual observer, if
///                 // it's a generic type
///   , <generics>? // options, give the generics type must use in the
///                 // implement, except `Item` and `Err` and host type.
///   , {where}?    // options, where bounds for the generics )
///
/// # Example
/// ```rust ignore
///  observer_proxy_impl!(Arc<Mutex<S>>, {lock().unwrap()}, S, <S>);
/// ```
pub(crate) macro observer_proxy_impl(
  $ty: ty
  , {$($name:tt $($parentheses:tt)?) .+}
  , $item: ident
  , $err: ident
  $(, $host_ty: ident)? $(, <$($generics: tt),*>)?
  $(, {where $($wty:ty : $bound: tt),*})?
) {
    impl<$($($generics ,)*)? $item, $err, $($host_ty)? >
      Observer<$item, $err> for $ty
    where
      $($host_ty: Observer<$item, $err>,)?
      $($($wty: $bound), *)?
    {
      next_proxy_impl!($item, $($name$($parentheses)?).+);
      error_proxy_impl!($err, $($name$($parentheses)?).+);
      complete_proxy_impl!($($name$($parentheses)?).+);
    }
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
macro observer_pointer_proxy_impl(
  $ty: ty, $item: ident, $err:ident $(, <$($generics: tt),*>)?)
{
  impl<$($($generics ,)*)? $item, $err> Observer<$item, $err> for $ty {

    #[inline]
    fn next(&mut self, value: $item) { (&mut **self).next(value); }

    #[inline]
    fn error(&mut self, err: $err) { (&mut **self).error(err); }

    #[inline]
    fn complete(&mut self) { (&mut **self).complete(); }
  }
}

// implement `Observer` for Box<dyn Observer<Item, Err> + 'a>
observer_pointer_proxy_impl!(
  Box<dyn Observer<Item, Err> + 'a>, Item, Err, <'a>);

// implement `Observer` for Box<dyn Observer<Item, Err> + Send + Sync>
observer_pointer_proxy_impl!(
  Box<dyn Observer<Item, Err> + Send + Sync>,
  Item,
  Err
);

// implement `Observer` for Box<dyn Publisher<Item, Err> + 'a>
observer_pointer_proxy_impl!(
  Box<dyn Publisher<Item, Err> + 'a>, Item, Err, <'a>);

// implement `Observer` for Box<dyn Publisher<Item, Err> + Send + Sync>
observer_pointer_proxy_impl!(
  Box<dyn Publisher<Item, Err> + Send + Sync>,
  Item,
  Err
);

observer_proxy_impl!(Arc<Mutex<S>>, { lock().unwrap() }, Item, Err, S);
observer_proxy_impl!(Rc<RefCell<S>>, { borrow_mut() }, Item, Err, S);

impl<Item, Err, T> Observer<Item, Err> for Vec<T>
where
  T: Publisher<Item, Err>,
  Item: Clone,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    self.drain_filter(|subscriber| {
      subscriber.next(value.clone());
      subscriber.is_closed()
    });
  }

  fn error(&mut self, err: Err) {
    self
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.clone()));
    self.clear();
  }

  fn complete(&mut self) {
    self.iter_mut().for_each(|subscriber| subscriber.complete());
    self.clear();
  }
}
