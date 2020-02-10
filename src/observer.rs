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

pub(crate) macro observer_proxy_impl($ty: ty, $host_ty: ident,
  <$($generics: tt),*>, $($name:ident $($parentheses:tt)?) .+) {
    impl<$($generics ,)* Item, Err> Observer<Item, Err> for $ty
    where
      $host_ty: Observer<Item, Err>
    {
      #[inline]
      fn next(&mut self, value: Item) { self.$($name $($parentheses)? ).+.next(value); }
      #[inline]
      fn error(&mut self, err: Err) { self.$($name $($parentheses)? ).+.error(err); }
      #[inline]
      fn complete(&mut self) { self.$($name $($parentheses)?).+.complete(); }
    }
}

macro observer_pointer_proxy_impl($ty: ty, $item: ident, $err:ident $(, <$($generics: tt),*>)?) {
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
observer_pointer_proxy_impl!(Box<dyn Observer<Item, Err> + 'a>, Item, Err, <'a>);

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

observer_proxy_impl!(Arc<Mutex<S>>, S, <S>, lock().unwrap());

observer_proxy_impl!(Rc<RefCell<S>>, S, <S>, borrow_mut());
