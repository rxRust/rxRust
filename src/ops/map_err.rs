use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

#[derive(Clone)]
pub struct MapErr<S, F> {
  pub source: S,
  pub func: F,
}

pub struct MapErrObserver<O, F> {
  observer: O,
  func: F,
}

impl<O, F, Item, Err, OutErr> Observer<Item, Err> for MapErrObserver<O, F>
where
  O: Observer<Item, OutErr>,
  F: FnOnce(Err) -> OutErr,
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, err: Err) {
    let out_err = (self.func)(err);
    self.observer.error(out_err);
  }

  fn complete(self) { self.observer.complete(); }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, F, OutErr> ObservableType for MapErr<S, F>
where
  S: ObservableType,
  F: FnOnce(S::Err) -> OutErr,
{
  type Item<'a>
    = S::Item<'a>
  where
    Self: 'a;
  type Err = OutErr;
}

impl<S, F, C, OutErr> CoreObservable<C> for MapErr<S, F>
where
  C: Context,
  S: CoreObservable<C::With<MapErrObserver<C::Inner, F>>>,
  F: FnOnce(S::Err) -> OutErr,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let MapErr { source, func } = self;
    let wrapped = context.transform(|observer| MapErrObserver { observer, func });
    source.subscribe(wrapped)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  struct AnyErrorObserver<F>(F);

  impl<F, Item, Err> Observer<Item, Err> for AnyErrorObserver<F>
  where
    F: FnMut(Item),
  {
    fn next(&mut self, value: Item) { (self.0)(value) }
    fn error(self, _err: Err) {}
    fn complete(self) {}
    fn is_closed(&self) -> bool { false }
  }

  #[rxrust_macro::test]
  fn test_map_err_transforms_error() {
    let error = Rc::new(RefCell::new(None));
    let error_clone = error.clone();

    Local::throw_err(1)
      .map_err(|e| format!("Error code: {}", e))
      .on_error(move |e| *error_clone.borrow_mut() = Some(e))
      .subscribe(|_| {});

    assert_eq!(*error.borrow(), Some("Error code: 1".to_string()));
  }

  #[rxrust_macro::test]
  fn test_map_err_passes_values() {
    let values = Rc::new(RefCell::new(Vec::new()));
    let values_clone = values.clone();
    let error = Rc::new(RefCell::new(None));
    let error_clone = error.clone();

    // o1 and o2 need to match the type of o3: Item=i32, Err=&str
    let o1 = Local::of(1)
      .map_err(|_: Infallible| "dummy")
      .box_it();
    let o2 = Local::of(2)
      .map_err(|_: Infallible| "dummy")
      .box_it();
    // o3 needs to match Item=i32
    let o3 = Local::throw_err("original error")
      .map(|_| 0)
      .box_it();

    let observables = vec![o1, o2, o3];

    // Manually create outer observable with Err=&str to match inner observables
    Local::from_iter(observables)
      .map_err(|_: Infallible| -> &str { unreachable!() })
      .concat_all()
      .map_err(|e| format!("mapped: {}", e))
      .on_error(move |e| *error_clone.borrow_mut() = Some(e))
      .subscribe_with(Local::<()>::lift(AnyErrorObserver(move |v| {
        values_clone.borrow_mut().push(v)
      })));

    assert_eq!(*values.borrow(), vec![1, 2]);
    assert_eq!(*error.borrow(), Some("mapped: original error".to_string()));
  }

  #[rxrust_macro::test]
  fn test_map_err_ignores_completion() {
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    Local::of(1)
      .map_err(|_| "should not happen")
      .on_complete(move || *completed_clone.borrow_mut() = true)
      // Local::of emits Infallible error, which map_err maps to &str.
      // So we need subscribe_with because Err is not Infallible anymore.
      .subscribe_with(Local::<()>::lift(AnyErrorObserver(|_| {})));

    assert!(*completed.borrow(), "Stream should complete normally even with map_err present");
  }
}
