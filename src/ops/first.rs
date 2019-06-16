use crate::{
  ops::{take::TakeOp, Take},
  prelude::*,
};
use std::{cell::RefCell, rc::Rc};

pub trait First {
  fn first(self) -> TakeOp<Self>
  where
    Self: Sized + Take,
  {
    self.take(1)
  }
}

impl<'a, O> First for O where O: Observable<'a> {}

pub trait FirstOr<'a> {
  fn first_or(self, default: Self::Item) -> FirstOrOp<TakeOp<Self>, Self::Item>
  where
    Self: Observable<'a>,
  {
    FirstOrOp {
      source: self.first(),
      default,
    }
  }
}

impl<'a, O> FirstOr<'a> for O where O: Observable<'a> {}

pub struct FirstOrOp<S, V> {
  source: S,
  default: V,
}

impl<'a, S, T> Observable<'a> for FirstOrOp<S, T>
where
  T: 'a,
  S: Observable<'a, Item = T>,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + FnMut(Self::Item) -> OState<Self::Err>,
  {
    let next = Rc::new(RefCell::new(next));
    let c_next = next.clone();
    let Self { source, default } = self;
    let mut subscription =
      source.subscribe_return_state(move |v| (&mut *(c_next.borrow_mut()))(v));

    let mut default = Some(default);
    let mut first = true;
    subscription.on_complete(move || {
      if first {
        first = false;
        let d = default.take().unwrap();
        (&mut *(next.borrow_mut()))(d);
      }
    });
    subscription
  }
}
