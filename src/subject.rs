use crate::{Observable, Observer, Subscription};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type NextPtr<'a, T> = *const (Fn(&T) + 'a);

pub(crate) struct Callbacks<'a, T, E> {
  on_next: Box<Fn(&T) + 'a>,
  on_complete: Option<Box<Fn() + 'a>>,
  on_error: Option<Box<Fn(&E) + 'a>>,
}

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

  fn subscribe<N>(self, next: N) -> Self::Unsubscribe
  where
    N: Fn(Self::Item) + 'a,
  {
    let next: Box<Fn(Self::Item)> = Box::new(next);
    // of course, we know Self::Item and &'a T is the same type, but
    // rust can't infer it, so, write an unsafe code to let rust know.
    let next: Box<(dyn for<'r> std::ops::Fn(&'r T) + 'a)> =
      unsafe { std::mem::transmute(next) };
    let ptr = next.as_ref() as NextPtr<'a, T>;
    let cbs = Callbacks {
      on_next: next,
      on_complete: None,
      on_error: None,
    };
    self.cbs.borrow_mut().push(cbs);

    SubjectSubscription {
      source: self,
      callback: ptr,
    }
  }
}

impl<'a, T: 'a, E: 'a> Subject<'a, T, E> {
  pub fn new() -> Self {
    Subject {
      cbs: Rc::new(RefCell::new(vec![])),
    }
  }

  /// Create a new subject from a stream, enabling multiple observers
  /// ("fork" the stream)
  pub fn from_stream<S>(stream: S) -> Self
  where
    S: Observable<'a, Item = T>,
  {
    let broadcast = Self::new();
    let for_next = broadcast.clone();
    stream.subscribe(move |v| {
      for_next.next(v);
    });

    broadcast
  }

  fn remove_callback(&mut self, ptr: NextPtr<'a, T>) {
    self
      .cbs
      .borrow_mut()
      .retain(|x| x.on_next.as_ref() as *const _ != ptr);
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
      if let Some(ref on_complete) = cbs.on_complete {
        on_complete();
      }
    }
    self.cbs.borrow_mut().clear();
  }

  fn error(self, err: Self::Err) {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      if let Some(ref on_error) = cbs.on_error {
        on_error(&err);
      }
    }
    self.cbs.borrow_mut().clear();
  }
}

pub struct SubjectSubscription<'a, T, E> {
  source: Subject<'a, T, E>,
  callback: NextPtr<'a, T>,
}

impl<'a, T: 'a, E: 'a> Subscription<'a> for SubjectSubscription<'a, T, E> {
  type Err = E;

  fn unsubscribe(mut self) { self.source.remove_callback(self.callback); }

  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a,
  {
    {
      let mut coll = self.source.cbs.borrow_mut();
      let cbs = coll
        .iter_mut()
        .find(|v| v.on_next.as_ref() as *const _ == self.callback);
      if let Some(cbs) = cbs {
        cbs.on_complete = Some(Box::new(complete));
      }
    }
    self
  }

  fn on_error<OE>(&mut self, err: OE) -> &mut Self
  where
    OE: Fn(&Self::Err) + 'a,
  {
    {
      let mut coll = self.source.cbs.borrow_mut();
      let cbs = coll
        .iter_mut()
        .find(|v| v.on_next.as_ref() as *const _ == self.callback);
      if let Some(cbs) = cbs {
        cbs.on_error = Some(Box::new(err));
      }
    }
    self
  }
}

#[test]
fn base_data_flow() {
  use std::cell::Cell;
  let i = Cell::new(0);
  let broadcast = Subject::<'_, i32, ()>::new();
  broadcast.clone().subscribe(|v: &i32| i.set(*v * 2));
  broadcast.next(1);
  assert_eq!(i.get(), 2);
}

#[test]
#[should_panic]
fn error() {
  let broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe(|_: &i32| {})
    .on_error(|e: &&str| panic!(*e));
  broadcast.next(1);

  broadcast.error("should panic!");
}
