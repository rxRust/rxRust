//! Reactive extensions library for Rust: a library for
//! [Reactive Programming](http://reactivex.io/) using
//! [Observable](crate::observable::Observable), to make
//! it easier to compose asynchronous or callback-based code.
#![recursion_limit = "256"]
#![feature(generic_associated_types)]
#![doc = include_str!("../README.md")]

#[cfg(test)]
extern crate float_cmp;

#[cfg(test)]
#[macro_use]
extern crate bencher;

#[cfg(test)]
pub mod test_scheduler;

pub mod impl_helper;
pub mod observable;
pub mod observer;
pub mod ops;
pub mod rc;
pub mod scheduler;
pub mod shared;
pub mod subject;
pub mod subscription;
pub mod type_hint;

pub mod prelude {

  pub use crate::observable;
  pub use crate::observable::*;
  pub use crate::observer;
  pub use crate::ops;
  pub use crate::rc::*;
  pub use crate::scheduler::*;
  pub use crate::shared;
  pub use crate::subject;
  pub use crate::subject::*;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use crate::type_hint::TypeHint;
  pub use observer::Observer;
  pub use shared::*;
}
