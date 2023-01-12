use super::of::CallableObservable;

pub fn start<F, Item>(func: F) -> CallableObservable<F>
where
  F: FnOnce() -> Item,
{
  CallableObservable(func)
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;

  #[test]
  fn it_shall_emit_closure_value() {
    let mut actual = 0;
    let mut is_completed = false;

    observable::start(|| 123)
      .on_complete(|| is_completed = true)
      .subscribe(|n| actual = n);

    assert_eq!(123, actual);
    assert!(is_completed);
  }

  #[derive(PartialEq, Debug)]
  struct S {
    i: i32,
    f: f32,
    s: String,
  }

  fn function() -> S {
    S { i: 1, f: 2.5, s: String::from("aString") }
  }

  #[test]
  fn it_shall_emit_function_value() {
    let expected = S { i: 1, f: 2.5, s: String::from("aString") };
    let mut actual = S { i: 0, f: 0.0, s: String::new() };
    let mut is_completed = false;

    observable::start(function)
      .on_complete(|| is_completed = true)
      .subscribe(|n| actual = n);

    assert_eq!(expected, actual);
    assert!(is_completed);
  }
}
