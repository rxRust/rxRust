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
  Item: PayloadCopy,
  Err: PayloadCopy,
{
  fn next(&mut self, value: Item) {
    self.drain_filter(|subscriber| {
      subscriber.next(value.payload_copy());
      subscriber.is_closed()
    });
  }

  fn error(&mut self, err: Err) {
    self
      .iter_mut()
      .for_each(|subscriber| subscriber.error(err.payload_copy()));
    self.clear();
  }

  fn complete(&mut self) {
    self.iter_mut().for_each(|subscriber| subscriber.complete());
    self.clear();
  }
}

/// There are situations in which rxRust needs to copy items/errors, for example
/// when you use subjects or cloned observables. All items/errors which are
/// copyable (= implement `Copy`) work in such situations out of the box.
/// Items/errors which implement just `Clone` but not `Copy` *don't* work out of
/// the box (because rxRust wants to prevent you from accidentally introducing
/// poorly performing observable chains). If you have such items/errors, you
/// might want to implement `PayloadCopy` for them. You can use it just like a
/// marker trait because there's a default method definition which simply calls
/// `clone()` to make the copy. However, take care to only implement it for
/// types that are cheap to clone! In multi-observer scenarios (e.g. when using
/// subjects), `payload_copy()` will be called for each single observer!
///
/// # Example
/// ```rust
/// use rxrust::prelude::*;
/// use std::rc::Rc;
///
/// #[derive(Clone)]
/// struct MyCloneableItem {
///   text: Rc<String>,
/// }
///
/// impl PayloadCopy for MyCloneableItem {}
/// ```
pub trait PayloadCopy: Clone {
  /// Should return a copy of the value. Unlike `Copy`, this can be more than
  /// just a bitwise copy. Unlike `Clone`, this should still be a cheap
  /// operation.
  #[must_use]
  fn payload_copy(&self) -> Self { self.clone() }
}

// Support all copyable types by default
impl<T: Copy> PayloadCopy for T {}
