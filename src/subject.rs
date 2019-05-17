use crate::{Observable, Observer, Subscription};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type CallbackPtr<'a, T> = *const (dyn for<'r> FnMut(&'r T) + 'a);

pub struct Subject<'a, T> {
  callbacks: Rc<RefCell<Vec<Box<FnMut(&T) + 'a>>>>,
}

impl<'a, T> Clone for Subject<'a, T> {
  fn clone(&self) -> Self {
    Subject {
      callbacks: self.callbacks.clone(),
    }
  }
}

impl<'a, T: 'a> Observable<'a> for Subject<'a, T> {
  type Item = &'a T;
  type Unsubcribe = Subscription<'a, T>;

  fn subscribe<O>(self, observer: O) -> Self::Unsubcribe
  where
    O: FnMut(Self::Item) + 'a,
  {
    let observer: Box<FnMut(Self::Item)> = Box::new(observer);
    // of course, we know Self::Item and &T is the same type, but
    // rust can't infer it, so, write an unsafe code to let rust know.
    let observer: Box<(dyn for<'r> std::ops::FnMut(&'r T) + 'a)> =
      unsafe { std::mem::transmute(observer) };
    let ptr = observer.as_ref() as CallbackPtr<T>;
    self.callbacks.borrow_mut().push(observer);

    Subscription::new(self, ptr)
  }
}

impl<'a, T: 'a> Subject<'a, T> {
  pub fn new() -> Subject<'a, T> {
    Subject {
      callbacks: Rc::new(RefCell::new(vec![])),
    }
  }

  /// Create a new subject from a stream, enabling multiple observers
  /// ("fork" the stream)
  pub fn from_stream<S>(stream: S) -> Self
  where
    S: Observable<'a, Item = T>,
  {
    let broadcast = Self::new();
    let clone = broadcast.clone();

    stream.subscribe(move |x| {
      clone.next(x);
    });
    broadcast
  }

  pub fn remove_callback(&mut self, ptr: CallbackPtr<T>) {
    self
      .callbacks
      .borrow_mut()
      .retain(|x| x.as_ref() as *const _ != ptr);
  }
}

impl<'a, T> Observer for Subject<'a, T> {
  type Item = T;

  fn next(&self, v: Self::Item) -> &Self {
    for observer in self.callbacks.borrow_mut().iter_mut() {
      observer(&v);
    }
    self
  }
}


#[test]
fn base_data_flow() {
  let mut i = 0;
  {
    let broadcast = Subject::new();
    broadcast.clone().subscribe(|v| i = *v * 2);
    broadcast.next(1);
  }
  assert_eq!(i, 2);
}