#![feature(external_doc, specialization, drain_filter, test, decl_macro)]
//! Reactive extensions library for Rust: a library for
//! [Reactive Programming](http://reactivex.io/) using
//! [Observable](crate::observable::Observable), to make
//! it easier to compose asynchronous or callback-based code.
#![doc(include = "../README.md")]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
extern crate float_cmp;

pub mod observable;
pub mod observer;
pub mod ops;
pub mod scheduler;
pub mod shared;
pub mod subject;
pub mod subscriber;
pub mod subscription;

pub mod prelude {
  pub use crate::observable;
  pub use crate::observable::*;
  pub use crate::observer;
  pub use crate::ops;
  pub use crate::scheduler::*;
  pub use crate::shared;
  pub use crate::subject;
  pub use crate::subject::{LocalSubject, SharedSubject, Subject};
  pub use crate::subscriber::Subscriber;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use observer::Observer;
  pub use shared::*;
}
