//! Example: rebuilding aggregates from an event log
//!
//! Account events flow through one connectable stream. `group_by` routes
//! them to a fold per account (`scan`), whose `last` value is the rebuilt
//! balance, while `materialize` turns every notification into data for an
//! audit log. `publish()` + `connect()` replay the synchronous log once every
//! consumer is attached.
//!
//! Run with `cargo run --example event_sourcing`.

use std::{cell::RefCell, collections::BTreeMap, convert::Infallible, rc::Rc};

use rxrust::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
  Deposited { account: &'static str, amount: i64 },
  Withdrawn { account: &'static str, amount: i64 },
}

impl Event {
  fn account(&self) -> &'static str {
    match self {
      Event::Deposited { account, .. } | Event::Withdrawn { account, .. } => account,
    }
  }
}

/// The fold: apply one event to a balance.
pub fn apply(balance: i64, event: Event) -> i64 {
  match event {
    Event::Deposited { amount, .. } => balance + amount,
    Event::Withdrawn { amount, .. } => balance - amount,
  }
}

pub const LOG: &[Event] = &[
  Event::Deposited { account: "a", amount: 100 },
  Event::Deposited { account: "b", amount: 40 },
  Event::Withdrawn { account: "a", amount: 30 },
  Event::Withdrawn { account: "b", amount: 15 },
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
  pub balances: BTreeMap<&'static str, i64>,
  pub audit: Vec<Notification<Event, Infallible>>,
}

pub fn run() -> Outcome {
  let balances = Rc::new(RefCell::new(BTreeMap::new()));
  let audit = Rc::new(RefCell::new(Vec::new()));

  // One subscription of the log feeds both the audit trail and the folds.
  // The log is synchronous, so consumers attach to the connectable first and
  // `connect()` replays it once everyone is listening; `share()` would have
  // connected on the first subscriber and finished before the second.
  let events = Local::from_iter(LOG.to_vec()).publish();

  let audit_sink = audit.clone();
  events
    .fork()
    .materialize()
    .subscribe(move |n| audit_sink.borrow_mut().push(n));

  let balances_sink = balances.clone();
  events
    .fork()
    .group_by(|e| e.account())
    .subscribe(move |group: Local<_>| {
      let account = group.inner().key;
      let sink = balances_sink.clone();
      group
        .scan(0, apply)
        .last()
        .subscribe(move |balance| {
          println!("{account}: {balance}");
          sink.borrow_mut().insert(account, balance);
        });
    });

  events.connect();

  Outcome { balances: balances.borrow().clone(), audit: audit.borrow().clone() }
}

fn main() {
  println!("{:#?}", run());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn balances_are_rebuilt_per_account_and_every_event_is_audited() {
    let outcome = run();
    assert_eq!(outcome.balances, BTreeMap::from([("a", 70), ("b", 25)]));
    // Four events plus the completion notification
    assert_eq!(outcome.audit.len(), 5);
    assert_eq!(outcome.audit[0], Notification::Next(LOG[0].clone()));
    assert_eq!(outcome.audit[4], Notification::Complete);
  }
}
