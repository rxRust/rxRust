use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type NextPtr<'a, T, E> = *const (dyn Fn(&T) -> OState<E> + 'a);

pub(crate) struct Callbacks<'a, T, E> {
  on_next: Box<dyn Fn(&T) -> OState<E> + 'a>,
  on_complete: Option<Box<dyn Fn() + 'a>>,
  on_error: Option<Box<dyn Fn(&E) + 'a>>,
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

impl<'a, T: 'a, Err: 'a> Subscribable<'a> for Subject<'a, T, Err> {
  type Item = T;
  type Err = Err;
  type Unsubscribable = SubjectSubscription<'a, T, Err>;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsubscribable {
    let on_next: Box<dyn Fn(&Self::Item) -> OState<Err>> = Box::new(next);
    let on_error = error.map(|e| {
      let e: Box<dyn Fn(&Self::Err)> = Box::new(e);
      e
    });
    let on_complete = complete.map(|c| {
      let c: Box<dyn Fn()> = Box::new(c);
      c
    });
    let ptr = on_next.as_ref() as NextPtr<'a, T, Err>;
    let cbs = Callbacks {
      on_next,
      on_complete,
      on_error,
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
    S: Subscribable<'a, Item = T, Err = E>,
  {
    let broadcast = Self::new();
    let for_next = broadcast.clone();
    let for_complete = broadcast.clone();
    let for_err = broadcast.clone();
    stream.subscribe_err_complete(
      move |v| {
        for_next.next(v);
      },
      move |err: &S::Err| {
        for_err.clone().error(err);
      },
      move || {
        for_complete.clone().complete();
      },
    );

    broadcast
  }

  fn remove_callback(&mut self, ptr: NextPtr<'a, T, E>) {
    self
      .cbs
      .borrow_mut()
      .retain(|x| x.on_next.as_ref() as *const _ != ptr);
  }
}

// completed return or unsubscribe called.
impl<'a, T, E> Observer for Subject<'a, T, E> {
  type Item = T;
  type Err = E;

  fn next(&self, v: &Self::Item) -> &Self {
    self.cbs.borrow_mut().drain_filter(|cb| {
      let mut stopped = false;
      match (cb.on_next)(&v) {
        OState::Complete => {
          if let Some(ref mut on_comp) = cb.on_complete {
            on_comp();
            stopped = true;
          }
        }
        OState::Err(err) => {
          if let Some(ref mut on_err) = cb.on_error {
            on_err(&err);
            stopped = true;
          }
        }
        _ => {}
      };
      stopped
    });
    self
  }

  fn complete(&mut self) {
    self.cbs.borrow().iter().for_each(|cbs| {
      if let Some(ref on_complete) = cbs.on_complete {
        on_complete();
      }
    });
    self.cbs.borrow_mut().clear();
  }

  fn error(&mut self, err: &Self::Err) {
    self.cbs.borrow().iter().for_each(|cbs| {
      if let Some(ref on_error) = cbs.on_error {
        on_error(err);
      }
    });
    self.cbs.borrow_mut().clear();
  }
}

pub struct SubjectSubscription<'a, T, E> {
  source: Subject<'a, T, E>,
  callback: NextPtr<'a, T, E>,
}

impl<'a, T, E> Clone for SubjectSubscription<'a, T, E> {
  fn clone(&self) -> Self {
    SubjectSubscription {
      source: self.source.clone(),
      callback: self.callback,
    }
  }
}

impl<'a, T: 'a, E: 'a> Subscription for SubjectSubscription<'a, T, E> {
  fn unsubscribe(&mut self) { self.source.remove_callback(self.callback); }
}

#[test]
fn base_data_flow() {
  use std::cell::Cell;
  let i = Cell::new(0);
  let broadcast = Subject::<'_, i32, ()>::new();
  broadcast.clone().subscribe(|v: &i32| i.set(*v * 2));
  broadcast.next(&1);
  assert_eq!(i.get(), 2);
}

#[test]
#[should_panic]
fn error() {
  let mut broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe_err(|_: &i32| {}, |e: &&'static str| panic!(*e));
  broadcast.next(&1);

  broadcast.error(&"should panic!");
}

#[test]
#[should_panic]
fn runtime_error() {
  let broadcast = Subject::new();
  let complete: Option<fn()> = None;
  broadcast.clone().subscribe_return_state(
    |_| OState::Err("runtime error"),
    Some(Box::new(|e: &&'static str| panic!(*e))),
    complete,
  );

  broadcast.next(&1);
}

#[test]
fn return_err_state() {
  let ec = std::cell::Cell::new(0);
  let broadcast = Subject::new();
  let complete: Option<fn()> = None;
  broadcast.clone().subscribe_return_state(
    |_| OState::Err("runtime error"),
    Some(Box::new(|_: &_| ec.set(ec.get() + 1))),
    complete,
  );

  broadcast.next(&1);
  assert_eq!(ec.get(), 1);
  // should stopped
  broadcast.next(&1);
  assert_eq!(ec.get(), 1);
}

#[test]
fn return_complete_state() {
  let cc = std::cell::Cell::new(0);
  let broadcast = Subject::new();
  let error: Option<fn(&())> = None;
  broadcast.clone().subscribe_return_state(
    |_| OState::Complete,
    error,
    Some(Box::new(|| cc.set(cc.get() + 1))),
  );

  broadcast.next(&1);
  assert_eq!(cc.get(), 1);
  // should stopped
  broadcast.next(&1);
  assert_eq!(cc.get(), 1);
}
