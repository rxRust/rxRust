use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer {
  type Item;
  type Err;
  fn next(&mut self, value: Self::Item);
  fn error(&mut self, err: Self::Err);
  fn complete(&mut self);
}

#[doc(hidden)]
#[macro_export]
macro_rules! next_proxy_impl {
    ($item: ident, $($name:tt $($parentheses:tt)?) .+) => {
  #[inline]
  fn next(&mut self, value: $item) {
    self.$($name$($parentheses)?).+.next(value);
  }
}
}

#[doc(hidden)]
#[macro_export]
macro_rules! error_proxy_impl {
    ($err: ident, $($name:tt $($parentheses:tt)?) .+) => {
  #[inline]
  fn error(&mut self, err: $err) {
    self.$($name$($parentheses)?).+.error(err);
  }
}
}

#[doc(hidden)]
#[macro_export]
macro_rules! complete_proxy_impl {
    ($($name:tt $($parentheses:tt)?) .+) => {
  #[inline]
  fn complete(&mut self) { self.$($name$($parentheses)?).+.complete(); }
}
}

impl<Item, Err, T> Observer for Arc<Mutex<T>>
where
  T: Observer<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) { self.lock().unwrap().next(value) }
  fn error(&mut self, err: Err) { self.lock().unwrap().error(err); }
  fn complete(&mut self) { self.lock().unwrap().complete(); }
}

impl<Item, Err, T> Observer for Rc<RefCell<T>>
where
  T: Observer<Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) { self.borrow_mut().next(value) }
  fn error(&mut self, err: Err) { self.borrow_mut().error(err); }
  fn complete(&mut self) { self.borrow_mut().complete(); }
}

impl<Item, Err, T> Observer for Box<T>
where
  T: Observer<Item = Item, Err = Err> + ?Sized,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    let s = &mut **self;
    s.next(value)
  }
  fn error(&mut self, err: Err) {
    let s = &mut **self;
    s.error(err);
  }
  fn complete(&mut self) {
    let s = &mut **self;
    s.complete();
  }
}
