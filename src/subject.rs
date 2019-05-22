use crate::{ErrComplete, Observable, Observer, Subscription};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type CallbackPtr<'a, T, E> = *const Callbacks<'a, T, E>;

pub(crate) struct Callbacks<'a, T, E> {
  on_next: Box<Fn(&T) + 'a>,
  on_err_or_complete: Box<Fn(&ErrComplete<E>) + 'a>,
}

#[derive(Default)]
pub struct Subject<'a, T, E> {
  cbs: Rc<RefCell<Vec<Callbacks<'a, T, E>>>>,
}

impl<'a, T, E> Clone for Subject<'a, T, E> {
  fn clone(&self) -> Self {
    Subject {
      cbs: self.cbs.clone(),
    }
  }
}

impl<'a, T: 'a, E: 'a> Observable<'a> for Subject<'a, T, E> {
  type Item = &'a T;
  type Err = E;
  type Unsubscribe = SubjectSubscription<'a, T, E>;

  fn subscribe<N, EC>(self, next: N, err_or_complete: EC) -> Self::Unsubscribe
  where
    N: Fn(Self::Item) + 'a,
    EC: 'a + Fn(&ErrComplete<E>),
  {
    let next: Box<Fn(Self::Item)> = Box::new(next);
    // of course, we know Self::Item and &'a T is the same type, but
    // rust can't infer it, so, write an unsafe code to let rust know.
    let next: Box<(dyn for<'r> std::ops::Fn(&'r T) + 'a)> =
      unsafe { std::mem::transmute(next) };
    let cbs = Callbacks {
      on_next: next,
      on_err_or_complete: Box::new(err_or_complete),
    };
    self.cbs.borrow_mut().push(cbs);
    let ptr = self.cbs.borrow().last().unwrap() as CallbackPtr<'a, T, E>;

    SubjectSubscription {
      source: self,
      callback: ptr,
    }
  }
}

impl<'a, T: 'a, E: 'a> Subject<'a, T, E> {
  pub fn new() -> Subject<'a, T, E> {
    Subject {
      cbs: Rc::new(RefCell::new(vec![])),
    }
  }

  /// Create a new subject from a stream, enabling multiple observers
  /// ("fork" the stream)
  pub fn from_stream<S>(stream: S) -> Self
  where
    S: Observable<'a, Item = T, Err = E>,
  {
    let broadcast = Self::new();
    let for_next = broadcast.clone();
    let for_ec = broadcast.clone();
    stream.subscribe(
      move |v| {
        for_next.next(v);
      },
      move |ec| for_ec.ec(ec),
    );

    broadcast
  }

  fn remove_callback(&mut self, ptr: CallbackPtr<'a, T, E>) {
    self.cbs.borrow_mut().retain(|x| x as *const _ != ptr);
  }

  fn ec(&self, ec: &ErrComplete<E>) {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      (cbs.on_err_or_complete)(&ec);
    }
  }
}

impl<'a, T, E> Observer for Subject<'a, T, E> {
  type Item = T;
  type Err = E;

  fn next(&self, v: Self::Item) -> &Self {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      (cbs.on_next)(&v);
    }
    self
  }

  fn complete(self) {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      (cbs.on_err_or_complete)(&ErrComplete::Complete);
    }
    self.cbs.borrow_mut().clear();
  }

  fn err(self, err: Self::Err) {
    let err = ErrComplete::Err(err);
    for cbs in self.cbs.borrow_mut().iter_mut() {
      (cbs.on_err_or_complete)(&err);
    }
    self.cbs.borrow_mut().clear();
  }
}

pub struct SubjectSubscription<'a, T, E> {
  source: Subject<'a, T, E>,
  callback: CallbackPtr<'a, T, E>,
}

impl<'a, T: 'a, E: 'a> Subscription for SubjectSubscription<'a, T, E> {
  fn unsubscribe(mut self) { self.source.remove_callback(self.callback); }
}

#[test]
fn base_data_flow() {
  use std::cell::Cell;
  let i = Cell::new(0);
  let broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe(|v| i.set(*v * 2), |_: &ErrComplete<()>| {});
  broadcast.next(1);
  assert_eq!(i.get(), 2);
}
