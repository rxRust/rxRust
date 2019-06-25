use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type NextPtr<'a, T, E> = *const (dyn FnMut(&T) -> OState<E> + 'a);

pub(crate) struct Callbacks<'a, T, E> {
  on_next: Box<dyn FnMut(&T) -> OState<E> + 'a>,
  on_complete: Option<Box<dyn FnMut() + 'a>>,
  on_error: Option<Box<dyn FnMut(&E) + 'a>>,
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
  type Item = T;
  type Err = E;
  type Unsubscribe = SubjectSubscription<'a, T, E>;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + FnMut(&Self::Item) -> OState<Self::Err>,
  {
    let next: Box<dyn FnMut(&Self::Item) -> OState<E>> = Box::new(next);
    let ptr = next.as_ref() as NextPtr<'a, T, E>;
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
    S: Observable<'a, Item = T, Err = E>,
  {
    let broadcast = Self::new();
    let for_next = broadcast.clone();
    let for_complete = broadcast.clone();
    let for_err = broadcast.clone();
    stream
      .subscribe(move |v| {
        for_next.next(v);
      })
      .on_complete(move || for_complete.clone().complete())
      .on_error(move |err| for_err.clone().error(err));

    broadcast
  }

  fn remove_callback(&mut self, ptr: NextPtr<'a, T, E>) {
    self
      .cbs
      .borrow_mut()
      .retain(|x| x.on_next.as_ref() as *const _ != ptr);
  }
}

// todo: need a strategy to remove callback when runtime error occur,
// completed return or unsubscribe called.
impl<'a, T, E> Observer for Subject<'a, T, E> {
  type Item = T;
  type Err = E;

  fn next(&self, v: &Self::Item) -> &Self {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      match (cbs.on_next)(&v) {
        OState::Complete => {
          if let Some(ref mut on_comp) = cbs.on_complete {
            on_comp();
          }
        }
        OState::Err(err) => {
          if let Some(ref mut on_err) = cbs.on_error {
            on_err(&err);
          }
        }
        _ => {}
      }
    }
    self
  }

  fn complete(self) {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      if let Some(ref mut on_complete) = cbs.on_complete {
        on_complete();
      }
    }
    self.cbs.borrow_mut().clear();
  }

  fn error(self, err: &Self::Err) {
    for cbs in self.cbs.borrow_mut().iter_mut() {
      if let Some(ref mut on_error) = cbs.on_error {
        on_error(err);
      }
    }
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

impl<'a, T: 'a, E: 'a> Subscription<'a> for SubjectSubscription<'a, T, E> {
  type Err = E;

  fn unsubscribe(mut self) { self.source.remove_callback(self.callback); }

  fn on_complete<C>(&mut self, mut complete: C) -> &mut Self
  where
    C: FnMut() + 'a,
  {
    {
      let mut coll = self.source.cbs.borrow_mut();
      let cbs = coll
        .iter_mut()
        .find(|v| v.on_next.as_ref() as *const _ == self.callback);
      if let Some(cbs) = cbs {
        let old_complete = cbs.on_complete.take();
        if let Some(mut o) = old_complete {
          cbs.on_complete.replace(Box::new(move || {
            o();
            complete();
          }));
        } else {
          cbs.on_complete.replace(Box::new(complete));
        }
      }
    }
    self
  }

  fn on_error<OE>(&mut self, mut err: OE) -> &mut Self
  where
    OE: FnMut(&Self::Err) + 'a,
  {
    {
      let mut coll = self.source.cbs.borrow_mut();
      let cbs = coll
        .iter_mut()
        .find(|v| v.on_next.as_ref() as *const _ == self.callback);
      if let Some(cbs) = cbs {
        let old_error = cbs.on_error.take();
        if let Some(mut o) = old_error {
          cbs.on_error.replace(Box::new(move |e| {
            o(&e);
            err(&e);
          }));
        } else {
          cbs.on_error.replace(Box::new(err));
        }
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
  broadcast.next(&1);
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
  broadcast.next(&1);

  broadcast.error(&"should panic!");
}

#[test]
#[should_panic]
fn runtime_error() {
  let broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe_return_state(|_| OState::Err("runtime error"))
    .on_error(|e: &&str| panic!(*e));

  broadcast.next(&1);
}

#[test]
fn mulit_on_error() {
  let ec = std::cell::Cell::new(0);
  let broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe(|_: &i32| {})
    .on_error(|_| ec.set(ec.get() + 1))
    .on_error(|_| ec.set(ec.get() + 1));

  broadcast.error(&1);

  assert_eq!(ec.get(), 2);
}

#[test]
fn multi_on_complete() {
  let cc = std::cell::Cell::new(0);
  let broadcast = Subject::<'_, _, &str>::new();
  broadcast
    .clone()
    .subscribe(|_: &i32| {})
    .on_complete(|| cc.set(cc.get() + 1))
    .on_complete(|| cc.set(cc.get() + 1));

  broadcast.complete();

  assert_eq!(cc.get(), 2);
}
