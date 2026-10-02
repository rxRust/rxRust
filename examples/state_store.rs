//! Example: a Redux-style state store
//!
//! Actions are dispatched on a `Subject`; a `scan` reducer folds them into
//! state; `share_replay(1)` multicasts the state and hands the current value
//! to late subscribers; selectors derive views with `distinct_until_changed`
//! so they only fire when their slice actually changes.
//!
//! Run with `cargo run --example state_store`.

use std::{cell::RefCell, convert::Infallible, rc::Rc};

use rxrust::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
  Increment,
  Decrement,
  Rename(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
  pub count: i64,
  pub name: String,
}

pub fn reduce(mut state: State, action: Action) -> State {
  match action {
    Action::Increment => state.count += 1,
    Action::Decrement => state.count -= 1,
    Action::Rename(name) => state.name = name,
  }
  state
}

/// What the selectors observed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
  pub counts: Vec<i64>,
  pub names: Vec<String>,
  pub late_subscriber_saw: Option<State>,
}

pub fn run() -> Outcome {
  let mut actions = Local::subject::<Action, Infallible>();

  // The store: every dispatched action becomes a new state, multicast to all
  // selectors, with the latest state replayed to whoever subscribes later.
  let state = actions
    .clone()
    .scan(State::default(), reduce)
    .share_replay(1);

  let counts = Rc::new(RefCell::new(Vec::new()));
  let names = Rc::new(RefCell::new(Vec::new()));
  let counts_sink = counts.clone();
  let names_sink = names.clone();

  // Selectors: each only fires when its slice changes.
  let _count_sub = state
    .clone()
    .map(|s: State| s.count)
    .distinct_until_changed()
    .subscribe(move |c| {
      println!("count -> {c}");
      counts_sink.borrow_mut().push(c);
    });
  let _name_sub = state
    .clone()
    .map(|s: State| s.name)
    .distinct_until_changed()
    .subscribe(move |n| {
      println!("name -> {n:?}");
      names_sink.borrow_mut().push(n);
    });

  for action in [
    Action::Increment,
    Action::Increment,
    Action::Rename("alpha".into()),
    Action::Rename("alpha".into()),
    Action::Decrement,
  ] {
    actions.next(action);
  }

  // A late subscriber gets the current state immediately.
  let late = Rc::new(RefCell::new(None));
  let late_sink = late.clone();
  let _late_sub = state.subscribe(move |s: State| *late_sink.borrow_mut() = Some(s));

  Outcome {
    counts: counts.borrow().clone(),
    names: names.borrow().clone(),
    late_subscriber_saw: late.borrow().clone(),
  }
}

fn main() {
  println!("{:#?}", run());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn selectors_fire_only_on_change_and_late_subscribers_get_current_state() {
    let outcome = run();
    assert_eq!(outcome.counts, vec![1, 2, 1]);
    assert_eq!(outcome.names, vec!["".to_string(), "alpha".to_string()]);
    assert_eq!(outcome.late_subscriber_saw, Some(State { count: 1, name: "alpha".to_string() }));
  }
}
