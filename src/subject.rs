use crate::ops::Fork;
use crate::prelude::*;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Default)]
pub struct Subject<'a, Item, Err> {
  cbs: Rc<
    RefCell<Vec<Rc<RefCell<Box<dyn Publisher<Item = Item, Err = Err> + 'a>>>>>,
  >,
  stopped: Rc<Cell<bool>>,
  _p: PhantomData<(Item, Err)>,
}

impl<'a, T, E> Clone for Subject<'a, T, E> {
  fn clone(&self) -> Self {
    Subject {
      cbs: self.cbs.clone(),
      stopped: self.stopped.clone(),
      _p: PhantomData,
    }
  }
}

impl<'a, Item: 'a, Err: 'a> ImplSubscribable<'a> for Subject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Box<dyn Subscription + 'a> {
    let mut subscriber = Subscriber::new(next);
    if error.is_some() {
      subscriber.on_error(error.unwrap());
    }
    if complete.is_some() {
      subscriber.on_complete(complete.unwrap());
    }

    let subscriber: Box<dyn Publisher<Item = Item, Err = Err> + 'a> =
      Box::new(subscriber);
    let subscriber = Rc::new(RefCell::new(subscriber));
    self.cbs.borrow_mut().push(subscriber.clone());

    Box::new(subscriber)
  }
}

impl<'a, 'b, Item: 'a, Err: 'a> Fork<'a> for Subject<'b, Item, Err> {
  type Output = Self;
  fn fork(&'a self) -> Self::Output { self.clone() }
}

impl<'a, Item: 'a, Err: 'a> Subject<'a, Item, Err> {
  pub fn new() -> Self {
    Subject {
      cbs: Rc::new(RefCell::new(vec![])),
      stopped: Rc::new(Cell::new(false)),
      _p: PhantomData,
    }
  }
}

impl<'a, Item, Err> Subscription
  for Rc<RefCell<Box<dyn Publisher<Item = Item, Err = Err> + 'a>>>
{
  #[inline]
  fn unsubscribe(&mut self) { self.borrow_mut().unsubscribe(); }
}

// completed return or unsubscribe called.
impl<'a, T, E> Observer for Subject<'a, T, E> {
  type Item = T;
  type Err = E;

  fn next(&self, v: &Self::Item) -> OState<Self::Err> {
    if self.stopped.get() {
      return OState::Complete;
    };
    self.cbs.borrow_mut().drain_filter(|subscriber| {
      let mut subscriber = subscriber.borrow_mut();
      match subscriber.next(&v) {
        OState::Complete => {
          subscriber.complete();
        }
        OState::Err(err) => {
          subscriber.error(&err);
        }
        _ => {}
      };
      subscriber.is_stopped()
    });

    if self.cbs.borrow().len() > 0 {
      OState::Next
    } else {
      OState::Complete
    }
  }

  fn complete(&mut self) {
    if self.stopped.get() {
      return;
    };
    self.cbs.borrow().iter().for_each(|subscriber| {
      subscriber.borrow_mut().complete();
    });
    self.cbs.borrow_mut().clear();
    self.stopped.set(true);
  }

  fn error(&mut self, err: &Self::Err) {
    if self.stopped.get() {
      return;
    };
    self.cbs.borrow().iter().for_each(|subscriber| {
      subscriber.borrow_mut().error(err);
    });
    self.cbs.borrow_mut().clear();
    self.stopped.set(true);
  }

  fn is_stopped(&self) -> bool { self.stopped.get() }
}

#[test]
fn base_data_flow() {
  use std::{cell::Cell, rc::Rc};
  let i = Rc::new(Cell::new(0));
  let ic = i.clone();
  let broadcast = Subject::<i32, ()>::new();
  broadcast.clone().subscribe(move |v: &i32| i.set(*v * 2));
  broadcast.next(&1);
  assert_eq!(ic.get(), 2);
}

#[test]
#[should_panic]
fn error() {
  let mut broadcast = Subject::new();
  broadcast
    .clone()
    .subscribe_err(|_: &i32| {}, |e: &_| panic!(*e));
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
    Some(Box::new(|e: &_| panic!(*e))),
    complete,
  );

  broadcast.next(&1);
}

#[test]
fn return_err_state() {
  use std::cell::Cell;
  let ec = Cell::new(0);
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
  use std::{cell::Cell, rc::Rc};
  let cc = Rc::new(Cell::new(0));
  let broadcast = Subject::new();
  let error: Option<fn(&())> = None;
  let b_c = cc.clone();
  broadcast.clone().subscribe_return_state(
    |_| OState::Complete,
    error,
    Some(Box::new(move || b_c.set(b_c.get() + 1))),
  );

  broadcast.next(&1);
  assert_eq!(cc.get(), 1);
  // should stopped
  broadcast.next(&1);
  assert_eq!(cc.get(), 1);
}

#[test]
fn unsubscribe() {
  use std::cell::Cell;
  let i = Cell::new(0);
  let subject = Subject::<'_, _, ()>::new();
  subject.clone().subscribe(|v| i.set(*v)).unsubscribe();
  subject.next(&0);
  assert_eq!(i.get(), 0);
}

#[test]
fn fork() {
  let subject = Subject::<'_, (), ()>::new();
  subject.fork().fork().fork().fork();
}
